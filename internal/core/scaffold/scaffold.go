// Package scaffold is the engine behind a tool's `init`, `update` and `check`:
// it copies an embedded file tree into a workspace, refreshes managed files
// without overwriting user edits (tracked by a manifest of content hashes), and
// validates the tree.
package scaffold

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/btdstudio/daiku/internal/core/settings"
	"github.com/btdstudio/daiku/internal/core/version"
	"github.com/btdstudio/daiku/internal/core/workspace"
)

// Engine scaffolds one tool's workspace.
type Engine struct {
	Tool string
	// FS holds the root tree to copy out, rooted at the workspace root.
	FS fs.FS
	// Rooms are optional sub-trees by name. A room's files live under <room>/ in
	// the workspace and share the root's manifest. Init writes only the root;
	// EnsureRoom adds a room, and a room exists once its folder does. Optional.
	Rooms map[string]fs.FS
	// Marker is a folder that identifies an existing workspace, so `update` can
	// rebuild a lost manifest without writing into an unrelated directory.
	Marker string
	// Seeds generates files that `init` writes once and `update` never touches,
	// such as settings and state. Optional.
	Seeds func() ([]Seed, error)
	// VCS is the workspace's core "vcs" setting. "" and settings.VCSGit scaffold
	// the whole tree. settings.VCSNone leaves out .gitattributes and .gitkeep
	// everywhere (init and update don't write, record or report them) but keeps
	// .gitignore, and init doesn't run `git init` and records "vcs": "none" in
	// the project settings.
	VCS string
}

// IsGitFile reports whether rel, a slash-separated path, is a git file: a
// .gitignore, .gitattributes or .gitkeep at any depth. A local workspace (VCS
// none) leaves out every git file except .gitignore, which it keeps so that
// machine-local files such as .<tool>/active and .<tool>/paths.json stay out of
// an enclosing git repo, or of one created later.
func IsGitFile(rel string) bool {
	switch path.Base(rel) {
	case ".gitignore", ".gitattributes", ".gitkeep":
		return true
	}
	return false
}

// omitWhenLocal reports whether a local workspace (vcs: none) leaves rel out:
// every git file except .gitignore, which still keeps .<tool>/active and
// .<tool>/paths.json out of an enclosing or later git repo.
func omitWhenLocal(rel string) bool {
	return IsGitFile(rel) && path.Base(rel) != ".gitignore"
}

func (e Engine) local() bool { return e.VCS == settings.VCSNone }

func (e Engine) checkVCS() error {
	if e.VCS == "" {
		return nil
	}
	return settings.CheckVCS(e.Tool, e.VCS)
}

// Seed is a generated file, written only when absent.
type Seed struct {
	Rel  string // slash-separated path relative to the workspace root
	Data []byte
}

// Manifest records the tool version and a sha256 per managed file, so `update`
// can tell which files the user has edited.
type Manifest struct {
	Version string            `json:"version"`
	Files   map[string]string `json:"files"` // relative path -> sha256 hex
}

// Options configure Scaffold.
type Options struct {
	NoGit bool
	Force bool
}

// Report describes what Scaffold did.
type Report struct {
	Written []string
	GitInit bool
}

// UpdateReport describes what Update did.
type UpdateReport struct {
	Updated   []string
	Skipped   []string
	Unchanged []string
}

// tree is one embedded tree and the folder its files go under in a workspace
// ("" for the root).
type tree struct {
	fsys   fs.FS
	prefix string
}

func (e Engine) root() tree { return tree{fsys: e.FS} }

// roomNames lists the rooms in name order, so walks are deterministic.
func (e Engine) roomNames() []string {
	names := make([]string, 0, len(e.Rooms))
	for name := range e.Rooms {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// validRoom reports whether a room name is one folder inside the workspace: not
// empty, no separators, and not "." or "..".
func validRoom(room string) bool {
	return room != "" && !strings.ContainsAny(room, `/\`) && room != "." && filepath.IsLocal(room)
}

// checkRooms refuses room names that aren't one folder in the workspace. Rooms
// are compiled in, so this guards against a programming error writing outside
// the workspace.
func (e Engine) checkRooms() error {
	for _, room := range e.roomNames() {
		if !validRoom(room) {
			return fmt.Errorf("%s: invalid room name %q", e.Tool, room)
		}
	}
	return nil
}

// roomExists reports whether a room's folder is in the workspace as a real
// folder. A symlink doesn't count, so Update and BuildManifest never write
// through one to wherever it points.
func roomExists(dir, room string) bool {
	info, err := os.Lstat(filepath.Join(dir, filepath.FromSlash(room)))
	return err == nil && info.IsDir() && info.Mode()&os.ModeSymlink == 0
}

// roomIsSymlink reports whether a room's path in the workspace is a symlink.
func roomIsSymlink(dir, room string) bool {
	info, err := os.Lstat(filepath.Join(dir, filepath.FromSlash(room)))
	return err == nil && info.Mode()&os.ModeSymlink != 0
}

// trees are the root and every room whose folder exists in dir.
func (e Engine) trees(dir string) []tree {
	out := []tree{e.root()}
	for _, room := range e.roomNames() {
		if roomExists(dir, room) {
			out = append(out, tree{fsys: e.Rooms[room], prefix: room})
		}
	}
	return out
}

// allTrees are the root and every room, whether or not it's in a workspace.
func (e Engine) allTrees() []tree {
	out := []tree{e.root()}
	for _, room := range e.roomNames() {
		out = append(out, tree{fsys: e.Rooms[room], prefix: room})
	}
	return out
}

// walk calls fn for every file of the given trees, with rel the file's
// slash-separated path in the workspace.
func (e Engine) walk(trees []tree, fn func(rel string, data []byte) error) error {
	for _, t := range trees {
		if err := e.walkTree(t, fn); err != nil {
			return err
		}
	}
	return nil
}

func (e Engine) walkTree(t tree, fn func(rel string, data []byte) error) error {
	return fs.WalkDir(t.fsys, ".", func(p string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		// Skip OS junk (macOS .DS_Store) so the tree and its manifest are
		// deterministic regardless of the build machine.
		if path.Base(p) == ".DS_Store" {
			return nil
		}
		// A local workspace leaves out git files, except .gitignore.
		if e.local() && omitWhenLocal(p) {
			return nil
		}
		data, err := fs.ReadFile(t.fsys, p)
		if err != nil {
			return err
		}
		return fn(path.Join(t.prefix, p), data)
	})
}

// keptDirs creates, under a local workspace, the folders that the tree's
// .gitkeep files exist to keep, since walk leaves those files out.
func (e Engine) keptDirs(dir string, trees []tree) error {
	if !e.local() {
		return nil
	}
	for _, t := range trees {
		err := fs.WalkDir(t.fsys, ".", func(p string, d fs.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if d.IsDir() || path.Base(p) != ".gitkeep" {
				return nil
			}
			return os.MkdirAll(filepath.Join(dir, filepath.FromSlash(path.Join(t.prefix, path.Dir(p)))), 0o755)
		})
		if err != nil {
			return err
		}
	}
	return nil
}

func hashBytes(data []byte) string {
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:])
}

// BuildManifest hashes every file of the root tree and of each room whose
// folder exists in dir.
func (e Engine) BuildManifest(dir string) (Manifest, error) {
	m := Manifest{Version: version.String(), Files: map[string]string{}}
	err := e.walk(e.trees(dir), func(rel string, data []byte) error {
		m.Files[rel] = hashBytes(data)
		return nil
	})
	if err != nil {
		return Manifest{}, err
	}
	return m, nil
}

func (e Engine) manifestPath(dir string) string {
	return filepath.Join(workspace.Dir(dir, e.Tool), "manifest.json")
}

// ReadManifest reads .<tool>/manifest.json.
func (e Engine) ReadManifest(dir string) (Manifest, error) {
	data, err := os.ReadFile(e.manifestPath(dir))
	if err != nil {
		return Manifest{}, err
	}
	var m Manifest
	if err := json.Unmarshal(data, &m); err != nil {
		return Manifest{}, err
	}
	if m.Files == nil {
		m.Files = map[string]string{}
	}
	return m, nil
}

// WriteManifest writes .<tool>/manifest.json.
func (e Engine) WriteManifest(dir string, m Manifest) error {
	data, err := json.MarshalIndent(m, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	if err := os.MkdirAll(filepath.Dir(e.manifestPath(dir)), 0o755); err != nil {
		return err
	}
	return os.WriteFile(e.manifestPath(dir), data, 0o644)
}

// Scaffold writes the tree into dir, skipping existing files (unless Force),
// initializing git when greenfield, and writing the seeds and manifest. A local
// workspace (VCS none) gets no git init and no git files, and its project
// settings record "vcs": "none".
func (e Engine) Scaffold(dir string, opts Options) (Report, error) {
	rep := Report{}
	if err := e.checkVCS(); err != nil {
		return rep, err
	}
	if err := e.checkRooms(); err != nil {
		return rep, err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return rep, err
	}
	if !opts.NoGit && !e.local() {
		if _, err := os.Stat(filepath.Join(dir, ".git")); os.IsNotExist(err) {
			if err := runGit(dir, "init"); err != nil {
				return rep, err
			}
			rep.GitInit = true
		}
	}
	err := e.walk([]tree{e.root()}, func(rel string, data []byte) error {
		dest := filepath.Join(dir, filepath.FromSlash(rel))
		if _, err := os.Stat(dest); err == nil && !opts.Force {
			return nil // skip existing user-owned file
		}
		if err := writeFile(dest, data); err != nil {
			return err
		}
		rep.Written = append(rep.Written, rel)
		return nil
	})
	if err != nil {
		return rep, err
	}
	if err := e.keptDirs(dir, []tree{e.root()}); err != nil {
		return rep, err
	}
	seeded, err := e.writeSeeds(dir)
	if err != nil {
		return rep, err
	}
	rep.Written = append(rep.Written, seeded...)
	if e.local() {
		created, err := e.recordLocal(dir)
		if err != nil {
			return rep, err
		}
		if created != "" {
			rep.Written = append(rep.Written, created)
		}
	}
	m, err := e.BuildManifest(dir)
	if err != nil {
		return rep, err
	}
	if err := e.WriteManifest(dir, m); err != nil {
		return rep, err
	}
	return rep, nil
}

// recordLocal sets "vcs": "none" in the project settings, creating the file when
// absent, so the workspace stays local after init. It returns the file's
// relative path when it created it.
func (e Engine) recordLocal(dir string) (string, error) {
	file := filepath.Join(workspace.Dir(dir, e.Tool), "settings.json")
	rel := filepath.ToSlash(filepath.Join(filepath.Base(workspace.Dir(dir, e.Tool)), "settings.json"))
	m := map[string]any{}
	created := true
	data, err := os.ReadFile(file)
	switch {
	case err == nil:
		created = false
		// Keep numbers as written, so re-encoding can't round them.
		dec := json.NewDecoder(strings.NewReader(string(data)))
		dec.UseNumber()
		var parsed any
		if err := dec.Decode(&parsed); err != nil {
			return "", fmt.Errorf("%s init: cannot parse settings %s: %v", e.Tool, file, err)
		}
		if dec.More() {
			return "", fmt.Errorf("%s init: cannot parse settings %s: trailing data after the object", e.Tool, file)
		}
		obj, ok := parsed.(map[string]any)
		if !ok || obj == nil {
			return "", fmt.Errorf("%s init: settings must be a JSON object: %s", e.Tool, file)
		}
		if obj["vcs"] == settings.VCSNone {
			return "", nil
		}
		m = obj
	case !os.IsNotExist(err):
		return "", err
	}
	m["vcs"] = settings.VCSNone
	// Encode leaves "&", "<" and ">" as written and ends with a newline.
	var out strings.Builder
	enc := json.NewEncoder(&out)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", "  ")
	if err := enc.Encode(m); err != nil {
		return "", err
	}
	if err := writeFile(file, []byte(out.String())); err != nil {
		return "", err
	}
	if created {
		return rel, nil
	}
	return "", nil
}

func (e Engine) writeSeeds(dir string) ([]string, error) {
	if e.Seeds == nil {
		return nil, nil
	}
	seeds, err := e.Seeds()
	if err != nil {
		return nil, err
	}
	var written []string
	for _, s := range seeds {
		dest := filepath.Join(dir, filepath.FromSlash(s.Rel))
		if _, err := os.Stat(dest); err == nil {
			continue
		}
		if err := writeFile(dest, s.Data); err != nil {
			return nil, err
		}
		written = append(written, s.Rel)
	}
	return written, nil
}

// Update refreshes managed files to the installed version. A file is overwritten
// only when its on-disk content still matches the recorded manifest hash;
// user-edited files are skipped and reported. A missing manifest is rebuilt:
// nothing on disk is recorded, so no differing file gets overwritten. In a local
// workspace (VCS none) git files are neither written nor recorded nor reported;
// ones already on disk are left alone.
func (e Engine) Update(dir string) (UpdateReport, error) {
	rep := UpdateReport{}
	if err := e.checkVCS(); err != nil {
		return rep, err
	}
	if err := e.checkRooms(); err != nil {
		return rep, err
	}
	cur, err := e.ReadManifest(dir)
	if errors.Is(err, fs.ErrNotExist) {
		if info, statErr := os.Stat(filepath.Join(dir, filepath.FromSlash(e.Marker))); statErr != nil || !info.IsDir() {
			return rep, fmt.Errorf("%s update: %s is not a %s workspace (run `%s init`)", e.Tool, dir, e.Tool, e.Tool)
		}
		cur, err = Manifest{Files: map[string]string{}}, nil
	}
	if err != nil {
		return rep, fmt.Errorf("%s update: %v", e.Tool, err)
	}
	next, err := e.BuildManifest(dir)
	if err != nil {
		return rep, err
	}
	if err := e.keptDirs(dir, e.trees(dir)); err != nil {
		return rep, err
	}
	for rel, nextHash := range next.Files {
		dest := filepath.Join(dir, filepath.FromSlash(rel))
		curData, err := os.ReadFile(dest)
		if err != nil {
			if err := e.writeManaged(dest, rel); err != nil {
				return rep, err
			}
			rep.Updated = append(rep.Updated, rel)
			continue
		}
		if hashBytes(curData) == nextHash {
			rep.Unchanged = append(rep.Unchanged, rel)
			continue
		}
		if prevHash, recorded := cur.Files[rel]; recorded && hashBytes(curData) == prevHash {
			if err := e.writeManaged(dest, rel); err != nil {
				return rep, err
			}
			rep.Updated = append(rep.Updated, rel)
			continue
		}
		// unrecorded (new file) or user-edited — skip to avoid clobbering
		rep.Skipped = append(rep.Skipped, rel)
	}
	if err := e.WriteManifest(dir, next); err != nil {
		return rep, err
	}
	return rep, nil
}

// writeManaged writes rel from the tree it belongs to: the room it's under, if
// any, else the root.
func (e Engine) writeManaged(dest, rel string) error {
	fsys, p := e.FS, rel
	// Room names have no separators, so at most one is a prefix of rel.
	for _, room := range e.roomNames() {
		if strings.HasPrefix(rel, room+"/") {
			fsys, p = e.Rooms[room], strings.TrimPrefix(rel, room+"/")
			break
		}
	}
	data, err := fs.ReadFile(fsys, p)
	if err != nil {
		return err
	}
	return writeFile(dest, data)
}

// Check validates the root tree and every room's, and returns a list of problems
// (empty when clean). It guards against template syntax leaking into the literal
// output.
func (e Engine) Check() []string {
	var problems []string
	if err := e.checkRooms(); err != nil {
		return []string{err.Error()}
	}
	if err := e.walk(e.allTrees(), func(rel string, data []byte) error {
		if strings.Contains(string(data), "{{") {
			problems = append(problems, rel+": contains template syntax")
		}
		return nil
	}); err != nil {
		problems = append(problems, err.Error())
	}
	return problems
}

func writeFile(dest string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
		return err
	}
	return os.WriteFile(dest, data, 0o644)
}

func runGit(dir string, args ...string) error {
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git %s: %v: %s", strings.Join(args, " "), err, out)
	}
	return nil
}
