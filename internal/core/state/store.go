package state

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/btdstudio/daiku/internal/core/changelog"
	"github.com/btdstudio/daiku/internal/core/settings"
	"github.com/btdstudio/daiku/internal/core/workspace"
)

// Schema is the STATE.json version. daiku starts at 1 and has no migrations: a
// newer file is refused, and an older one can't exist.
const Schema = 1

// Loop is one kind of change a Store holds: its phases, the names its changes
// may have, and where their folders live.
type Loop struct {
	Machine Machine
	// Name is the grammar for this loop's change names. It must keep a name from
	// escaping its folder. Optional; nil means PlanNameRe.
	Name *regexp.Regexp
	// Dir holds this loop's change folders directly, relative to the workspace
	// root, as <Dir>/<change>/. NextID and Known scan it. Optional when Folder is
	// set.
	Dir string
	// Folder overrides <Dir>/<change> for a layout that needs fields of the
	// record, such as its repo. It returns a path relative to the workspace root.
	// Optional.
	Folder func(c Change) (string, error)
	// ChangeDirs are other folders holding a <dir>/<change>/ per change, relative
	// to the workspace root. Gates commit these along with the change's folder.
	ChangeDirs []string
}

// Store persists changes at the workspace root: a committed STATE.json and
// LOG.md holding every loop's changes, each change's folder as its loop places
// it, and a local active-change pointer in .<tool>/active.
type Store struct {
	// Tool prefixes messages and names the .<tool>/ folder.
	Tool string
	// Loops are the kinds of change this store holds, by name. Every change
	// records one of these names in its Loop.
	Loops map[string]Loop
	// Commit makes each transition commit STATE.json and LOG.md, so an approval
	// can't sit uncommitted.
	Commit bool
	// VCS is the workspace's core "vcs" setting. "" and settings.VCSGit keep the
	// behavior above; settings.VCSNone means the workspace is plain local files:
	// transitions never run git, never warn, and change folders get no .gitkeep.
	VCS string
	// Now is the clock for timestamps. Optional; defaults to time.Now.
	Now func() time.Time
	// Warn receives warnings. Optional; defaults to os.Stderr.
	Warn io.Writer
	// Preflight runs before STATE.json is read, after the built-in refusal of
	// zumen and tomo workspaces, to refuse other layouts. Optional.
	Preflight func(root string) error
	// Refresh runs after a transition saves STATE.json and before it records, so
	// files it rewrites inside the change's folders go into the same commit.
	// Optional. Advance and Reopen call it. An error is printed as a warning; it
	// doesn't fail the transition.
	Refresh func(root string, c Change) error
}

type stateFile struct {
	Schema  int                `json:"schema"`
	Changes map[string]*Change `json:"changes"`
}

func (s Store) tool() string { return s.Tool }

// loop returns the named loop, with its Name rule defaulted.
func (s Store) loop(name string) (Loop, error) {
	l, ok := s.Loops[name]
	if !ok {
		return Loop{}, fmt.Errorf("%s: unknown loop %q", s.tool(), name)
	}
	if l.Name == nil {
		l.Name = PlanNameRe
	}
	return l, nil
}

// loopOf returns the loop a change records.
func (s Store) loopOf(c Change) (Loop, error) {
	l, err := s.loop(c.Loop)
	if err != nil {
		return Loop{}, fmt.Errorf("%s: unknown loop %q for change %q", s.tool(), c.Loop, c.Name)
	}
	return l, nil
}

// loopNames lists the loops in name order, so anything iterating them is
// deterministic.
func (s Store) loopNames() []string {
	names := make([]string, 0, len(s.Loops))
	for name := range s.Loops {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func (s Store) now() time.Time {
	if s.Now != nil {
		return s.Now()
	}
	return time.Now()
}

func (s Store) warn() io.Writer {
	if s.Warn != nil {
		return s.Warn
	}
	return os.Stderr
}

// local reports whether the workspace is declared local (vcs: none).
func (s Store) local() bool { return s.VCS == settings.VCSNone }

// commits reports whether transitions commit: Commit is on and the workspace is
// not local.
func (s Store) commits() bool { return s.Commit && !s.local() }

func (s Store) checkVCS() error {
	if s.VCS == "" {
		return nil
	}
	return settings.CheckVCS(s.tool(), s.VCS)
}

func stamp(t time.Time) string { return t.UTC().Format("2006-01-02T15:04:05.000Z") }

// StatePath is the committed state file.
func (s Store) StatePath(root string) string { return filepath.Join(root, "STATE.json") }

// LogPath is the committed log.
func (s Store) LogPath(root string) string { return filepath.Join(root, "LOG.md") }

// relDir is a change's folder relative to the workspace root, slash-separated:
// its loop's Folder, or <Dir>/<name>. The name must match the loop's grammar, and
// the folder must stay inside the workspace.
func (s Store) relDir(c Change, l Loop) (string, error) {
	if !l.Name.MatchString(c.Name) {
		return "", fmt.Errorf("%s: invalid change name %q", s.tool(), c.Name)
	}
	rel := path.Join(filepath.ToSlash(l.Dir), c.Name)
	if l.Folder != nil {
		got, err := l.Folder(c)
		if err != nil {
			return "", err
		}
		rel = filepath.ToSlash(got)
	}
	if !filepath.IsLocal(filepath.FromSlash(rel)) {
		return "", fmt.Errorf("%s: change %q has a folder outside the workspace: %s", s.tool(), c.Name, rel)
	}
	return path.Clean(rel), nil
}

// ChangeDir is a change's folder, as its loop places it.
func (s Store) ChangeDir(root string, c Change) (string, error) {
	l, err := s.loopOf(c)
	if err != nil {
		return "", err
	}
	rel, err := s.relDir(c, l)
	if err != nil {
		return "", err
	}
	return filepath.Join(root, filepath.FromSlash(rel)), nil
}

var changeIDRe = regexp.MustCompile(`^(\d+)-`)

// PlanNameRe is the grammar for a numbered change: a numeric id, a hyphen, then
// the kebab-case slug Slugify produces. It keeps a name from escaping its folder
// via `..` or a separator. A Loop without a Name rule uses it.
var PlanNameRe = regexp.MustCompile(`^(\d+)-[a-z0-9]+(?:-[a-z0-9]+)*$`)

// NextID is the next sequential change id in a loop: the highest numeric `<id>-`
// prefix among that loop's folders under its Dir and its changes recorded in
// STATE.json, plus one. Errors from reading either are returned, never treated
// as empty: a record whose folder is gone must still keep its id from being
// reused. A loop without a Dir has no numbered changes.
func (s Store) NextID(root, loop string) (int, error) {
	l, err := s.loop(loop)
	if err != nil {
		return 0, err
	}
	if l.Dir == "" {
		return 0, fmt.Errorf("%s: loop %s has no numbered changes", s.tool(), loop)
	}
	changes, err := s.ReadChanges(root)
	if err != nil {
		return 0, err
	}
	max := 0
	for name, c := range changes {
		if c.Loop == loop {
			max = maxID(max, name)
		}
	}
	dir := filepath.Join(root, filepath.FromSlash(l.Dir))
	entries, err := os.ReadDir(dir)
	if err != nil && !os.IsNotExist(err) {
		return 0, fmt.Errorf("%s: cannot read %s: %v", s.tool(), dir, err)
	}
	for _, e := range entries {
		max = maxID(max, e.Name())
	}
	return max + 1, nil
}

// maxID is the higher of max and name's `<id>-` prefix. A name without one
// leaves max as is.
func maxID(max int, name string) int {
	if m := changeIDRe.FindStringSubmatch(name); m != nil {
		if n, err := strconv.Atoi(m[1]); err == nil && n > max {
			return n
		}
	}
	return max
}

var slugNonAlnum = regexp.MustCompile(`[^a-z0-9]+`)

// Slugify lowercases text to a short kebab-case slug, so change folders read as
// <id>-<slug>.
func Slugify(text string) string {
	slug := slugNonAlnum.ReplaceAllString(strings.ToLower(text), "-")
	if r := []rune(strings.Trim(slug, "-")); len(r) > 40 {
		slug = string(r[:40])
	}
	// Trim after cutting too: a cut just past a "-" would leave a trailing one,
	// which isn't a valid change name.
	return strings.Trim(slug, "-")
}

// NewChange returns an empty change.
func NewChange(name string) Change {
	return Change{Name: name, Gates: map[string]string{}, Sources: []string{}}
}

// ReadChanges returns every recorded change (empty when STATE.json is absent). An
// unreadable or unparseable file is an error, never "no changes": a save writes
// this map back, so treating it as empty would wipe every record. So is a change
// with no loop, or with a loop the store doesn't have.
func (s Store) ReadChanges(root string) (map[string]*Change, error) {
	if err := workspace.CheckLegacy(root); err != nil {
		return nil, err
	}
	if s.Preflight != nil {
		if err := s.Preflight(root); err != nil {
			return nil, err
		}
	}
	p := s.StatePath(root)
	data, err := os.ReadFile(p)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]*Change{}, nil
		}
		return nil, fmt.Errorf("%s: cannot read %s: %v", s.tool(), p, err)
	}
	var sf stateFile
	if err := json.Unmarshal(data, &sf); err != nil {
		return nil, fmt.Errorf("%s: cannot parse %s: %v", s.tool(), p, err)
	}
	if sf.Schema > Schema {
		return nil, fmt.Errorf("%s: %s has schema %d, newer than this version supports (%d) — upgrade %s", s.tool(), p, sf.Schema, Schema, s.tool())
	}
	if sf.Changes == nil {
		return map[string]*Change{}, nil
	}
	for name, c := range sf.Changes {
		if c == nil {
			delete(sf.Changes, name)
			continue
		}
		c.Name = name
		if c.Loop == "" {
			return nil, fmt.Errorf("%s: %s: change %q records no loop", s.tool(), p, name)
		}
		// A change in a loop this store doesn't know would be invisible to Known
		// and unusable everywhere else, so refuse the file instead.
		if _, ok := s.Loops[c.Loop]; !ok {
			return nil, fmt.Errorf("%s: %s: change %q records unknown loop %q", s.tool(), p, name, c.Loop)
		}
		if c.Gates == nil {
			c.Gates = map[string]string{}
		}
		for phase, status := range c.Gates {
			if status == "" {
				delete(c.Gates, phase)
			}
		}
		if c.Sources == nil {
			c.Sources = []string{}
		}
	}
	return sf.Changes, nil
}

// resolve returns the recorded change, or for an unrecorded name, a new change
// in the loop whose Dir holds a folder of that name (Loop "" if none does).
func (s Store) resolve(root, name string, changes map[string]*Change) Change {
	if c := changes[name]; c != nil {
		return *c
	}
	c := NewChange(name)
	for _, loop := range s.loopNames() {
		l, _ := s.loop(loop)
		if l.Dir == "" || !l.Name.MatchString(name) {
			continue
		}
		if realDir(filepath.Join(root, filepath.FromSlash(l.Dir), name)) {
			c.Loop = loop
			break
		}
	}
	return c
}

// realDir reports whether p is a directory and not a symlink.
func realDir(p string) bool {
	info, err := os.Lstat(p)
	return err == nil && info.IsDir() && info.Mode()&os.ModeSymlink == 0
}

// Load returns the active change. Its Name is "" when none is active.
func (s Store) Load(root string) (Change, error) {
	changes, err := s.ReadChanges(root)
	if err != nil {
		return Change{}, err
	}
	name, err := workspace.ReadActive(root, s.tool())
	if err != nil {
		return Change{}, err
	}
	return s.resolve(root, name, changes), nil
}

func (s Store) write(root string, c *Change) error {
	c.UpdatedAt = stamp(s.now())
	changes, err := s.ReadChanges(root)
	if err != nil {
		return err
	}
	if c.Gates == nil {
		c.Gates = map[string]string{}
	}
	if c.Sources == nil {
		c.Sources = []string{}
	}
	changes[c.Name] = c
	buf, err := json.MarshalIndent(stateFile{Schema: Schema, Changes: changes}, "", "  ")
	if err != nil {
		return err
	}
	return workspace.WriteFile(root, s.StatePath(root), append(buf, '\n'), 0o644)
}

// CheckStateFiles refuses when STATE.json, the active pointer or LOG.md can't be
// written safely (see workspace.CheckPath). Transitions run it before their first
// write, so a refusal leaves the workspace as it was instead of a gate recorded
// in STATE.json but never logged or committed.
func (s Store) CheckStateFiles(root string) error {
	for _, p := range []string{s.StatePath(root), workspace.ActivePath(root, s.tool()), s.LogPath(root)} {
		if err := workspace.CheckPath(root, p); err != nil {
			return err
		}
	}
	return nil
}

// Save stamps updatedAt, writes the change's record (preserving every other
// change's), and makes it the active change. It writes nothing when a state file
// is unsafe to write.
func (s Store) Save(root string, c *Change) error {
	if err := s.CheckStateFiles(root); err != nil {
		return err
	}
	if err := s.write(root, c); err != nil {
		return err
	}
	return workspace.WriteActive(root, s.tool(), c.Name)
}

// KnownChange is one change annotated with any recorded progress.
type KnownChange struct {
	Name   string            `json:"name"`
	Loop   string            `json:"loop"`
	ID     int               `json:"id"` // the numeric prefix; 0 for a change without one
	Phase  string            `json:"phase"`
	Status string            `json:"status"`
	Gates  map[string]string `json:"gates"`
	Repos  []string          `json:"repos,omitempty"`
}

// Known lists every loop's changes, loops in name order. A loop with a Dir lists
// the numbered folders under it, sorted by id, with any recorded progress; a loop
// without one lists its recorded changes, sorted by name.
func (s Store) Known(root string) ([]KnownChange, error) {
	changes, err := s.ReadChanges(root)
	if err != nil {
		return nil, err
	}
	out := []KnownChange{}
	for _, loop := range s.loopNames() {
		l, _ := s.loop(loop)
		var group []KnownChange
		if l.Dir != "" {
			entries, err := os.ReadDir(filepath.Join(root, filepath.FromSlash(l.Dir)))
			if err != nil {
				continue
			}
			for _, e := range entries {
				m := changeIDRe.FindStringSubmatch(e.Name())
				if m == nil {
					continue
				}
				id, _ := strconv.Atoi(m[1])
				kc := KnownChange{Name: e.Name(), Loop: loop, ID: id, Gates: map[string]string{}}
				if c := changes[e.Name()]; c != nil && c.Loop == loop {
					kc.Phase, kc.Status, kc.Gates, kc.Repos = c.Phase, c.Status, c.Gates, c.Repos
				}
				group = append(group, kc)
			}
			sort.SliceStable(group, func(i, j int) bool { return group[i].ID < group[j].ID })
		} else {
			for name, c := range changes {
				if c.Loop != loop {
					continue
				}
				group = append(group, KnownChange{Name: name, Loop: loop, Phase: c.Phase, Status: c.Status, Gates: c.Gates, Repos: c.Repos})
			}
			sort.Slice(group, func(i, j int) bool { return group[i].Name < group[j].Name })
		}
		out = append(out, group...)
	}
	return out, nil
}

// checkFolder holds a change to its loop's rules: a name that can't escape its
// folder, and a real folder reached without passing through a symlink.
func (s Store) checkFolder(root string, c Change) error {
	unknown := fmt.Errorf(`%s: unknown change "%s"`, s.tool(), c.Name)
	l, err := s.loopOf(c)
	if err != nil {
		return unknown
	}
	rel, err := s.relDir(c, l)
	if err != nil {
		return unknown
	}
	if workspace.ThroughSymlink(root, rel) != "" || !realDir(filepath.Join(root, filepath.FromSlash(rel))) {
		return unknown
	}
	return nil
}

// Switch points the workspace at a change and resumes its machine. The change's
// folder must exist and its name must match its loop's grammar. An unrecorded
// folder under a loop's Dir is recorded in that loop. Switching to a recorded
// change moves only the local pointer, leaving STATE.json untouched.
func (s Store) Switch(root, name string) (Change, error) {
	changes, err := s.ReadChanges(root)
	if err != nil {
		return Change{}, err
	}
	c := s.resolve(root, name, changes)
	if err := s.checkFolder(root, c); err != nil {
		return Change{}, err
	}
	if changes[name] != nil {
		return c, workspace.WriteActive(root, s.tool(), name)
	}
	return c, s.Save(root, &c)
}

// SeedOptions configure Seed.
type SeedOptions struct {
	Sources []string
	// Repos are logical repo names this change works on.
	Repos []string
	// Skip maps skippable phases to the reason they don't apply to this change.
	Skip map[string]string
	// Paths are extra files to commit with the new change, relative to root, such
	// as an intake file its record points at.
	Paths []string
}

// Seed creates a change folder in a loop and records its initial machine (the
// first phase at draft), making it the active change. It refuses a name that
// doesn't match the loop's grammar, and a name STATE.json already records, in
// any loop.
func (s Store) Seed(root, name, loop string, opts SeedOptions) (Change, error) {
	l, err := s.loop(loop)
	if err != nil {
		return Change{}, err
	}
	if !l.Name.MatchString(name) {
		return Change{}, fmt.Errorf("%s: invalid change name %q", s.tool(), name)
	}
	if err := s.checkVCS(); err != nil {
		return Change{}, err
	}
	// Before the change folder is made, so a refusal leaves no folder behind.
	if err := s.CheckStateFiles(root); err != nil {
		return Change{}, err
	}
	m := l.Machine
	if len(m.Phases) == 0 {
		return Change{}, fmt.Errorf("%s: machine has no phases", s.tool())
	}
	// Check before writing anything: Save would replace an existing record, and
	// a refused seed must leave no folder behind.
	changes, err := s.ReadChanges(root)
	if err != nil {
		return Change{}, err
	}
	if _, ok := changes[name]; ok {
		return Change{}, fmt.Errorf("%s: change %q already exists in %s", s.tool(), name, s.StatePath(root))
	}
	c := NewChange(name)
	c.Loop = loop
	if opts.Sources != nil {
		c.Sources = opts.Sources
	}
	c.Repos = opts.Repos
	rel, err := s.relDir(c, l)
	if err != nil {
		return Change{}, err
	}
	first := m.Phases[0].Name
	lines := []string{name + ": created"}
	for _, p := range m.Phases {
		if reason, ok := opts.Skip[p.Name]; ok {
			if err := m.SetGate(&c, p.Name, Skipped, reason); err != nil {
				return Change{}, err
			}
			lines = append(lines, transitionLine(name, p.Name, Skipped, reason))
		}
	}
	if err := m.SetGate(&c, first, Draft, ""); err != nil {
		return Change{}, err
	}
	if link := workspace.ThroughSymlink(root, rel); link != "" {
		return Change{}, fmt.Errorf("%s: won't create %s: %s is a symlink, which could lead outside the workspace", s.tool(), rel, link)
	}
	dir := filepath.Join(root, filepath.FromSlash(rel))
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return Change{}, err
	}
	var paths []string
	if !s.local() {
		// Git drops empty folders, and a clone needs this one to switch to the change.
		keep := filepath.Join(dir, ".gitkeep")
		if err := workspace.WriteFile(root, keep, nil, 0o644); err != nil {
			return Change{}, err
		}
		paths = append(paths, path.Join(rel, ".gitkeep"))
	}
	if err := s.Save(root, &c); err != nil {
		return Change{}, err
	}
	paths = append(paths, opts.Paths...)
	return c, s.Record(root, lines, paths...)
}

func transitionLine(name, phase, status, reason string) string {
	line := name + ": " + phase + " → " + status
	if reason != "" {
		line += " (" + reason + ")"
	}
	return line
}

// active returns the active change and its loop.
func (s Store) active(root string) (Change, Loop, error) {
	c, err := s.Load(root)
	if err != nil {
		return Change{}, Loop{}, err
	}
	if c.Name == "" {
		return Change{}, Loop{}, fmt.Errorf("%s: no active change — run `%s switch <change>` first", s.tool(), s.tool())
	}
	// The pointer is local, so a branch switch can leave it naming a change that
	// doesn't exist here.
	if c.Loop == "" {
		// Unrecorded, and no loop holds a valid folder of that name. Say whether
		// something is there, so a crafted pointer reads as invalid, not missing.
		for _, loop := range s.loopNames() {
			if l, _ := s.loop(loop); l.Dir != "" {
				if _, err := os.Lstat(filepath.Join(root, filepath.FromSlash(l.Dir), c.Name)); err == nil {
					return Change{}, Loop{}, fmt.Errorf("%s: active change %q isn't a valid change folder — run `%s switch <change>`", s.tool(), c.Name, s.tool())
				}
			}
		}
		return Change{}, Loop{}, fmt.Errorf("%s: active change %q has no folder — run `%s switch <change>`", s.tool(), c.Name, s.tool())
	}
	l, err := s.loopOf(c)
	if err != nil {
		return Change{}, Loop{}, err
	}
	// The pointer is a plain file, so hold it to the same rules as Switch: a name
	// that can't escape its folder, and a real folder, not a symlink.
	rel, err := s.relDir(c, l)
	if err != nil {
		return Change{}, Loop{}, fmt.Errorf("%s: active change %q isn't a valid change folder — run `%s switch <change>`", s.tool(), c.Name, s.tool())
	}
	if _, err := os.Stat(filepath.Join(root, filepath.FromSlash(rel))); err != nil {
		return Change{}, Loop{}, fmt.Errorf("%s: active change %q has no folder %s/ — run `%s switch <change>`", s.tool(), c.Name, rel, s.tool())
	}
	if err := s.checkFolder(root, c); err != nil {
		return Change{}, Loop{}, fmt.Errorf("%s: active change %q isn't a valid change folder — run `%s switch <change>`", s.tool(), c.Name, s.tool())
	}
	return c, l, nil
}

// Advance sets a gate on the active change, runs Refresh, logs it along with any
// notes, and commits, including the change's folders, so an approval can't be
// recorded while the document it approved stays uncommitted. Enforcing the loop
// order is the caller's job, through CheckGate, since whether a violation blocks
// is its policy.
func (s Store) Advance(root, phase, status, reason string, notes ...string) (Change, error) {
	c, l, err := s.active(root)
	if err != nil {
		return Change{}, err
	}
	if err := l.Machine.SetGate(&c, phase, status, reason); err != nil {
		return Change{}, err
	}
	if err := s.Save(root, &c); err != nil {
		return Change{}, err
	}
	lines := []string{transitionLine(c.Name, phase, status, reason)}
	for _, note := range notes {
		lines = append(lines, c.Name+": "+note)
	}
	s.refresh(root, c)
	return c, s.Record(root, lines, s.changePaths(root, c, l)...)
}

// Reopen moves the active change back to a phase, runs Refresh, logs it and
// commits, including the change's folders, as Advance does.
func (s Store) Reopen(root, phase, reason string) (Change, error) {
	c, l, err := s.active(root)
	if err != nil {
		return Change{}, err
	}
	if err := l.Machine.Reopen(&c, phase, reason); err != nil {
		return Change{}, err
	}
	if err := s.Save(root, &c); err != nil {
		return Change{}, err
	}
	s.refresh(root, c)
	return c, s.Record(root, []string{c.Name + ": reopened " + phase + " (" + reason + ")"}, s.changePaths(root, c, l)...)
}

// changePaths lists a change's folders, relative to root: its own folder, and
// <dir>/<change> for each of its loop's ChangeDirs that exists.
func (s Store) changePaths(root string, c Change, l Loop) []string {
	var paths []string
	if rel, err := s.relDir(c, l); err == nil {
		paths = append(paths, rel)
	}
	for _, dir := range l.ChangeDirs {
		if _, err := os.Stat(filepath.Join(root, dir, c.Name)); err == nil {
			paths = append(paths, filepath.ToSlash(filepath.Join(dir, c.Name)))
		}
	}
	return paths
}

// refresh runs Refresh, if it's set, and turns an error into a warning.
func (s Store) refresh(root string, c Change) {
	if s.Refresh == nil {
		return
	}
	if err := s.Refresh(root, c); err != nil {
		fmt.Fprintf(s.warn(), "warning: %v\n", err)
	}
}

// CloseChange closes a change by name, logs it and commits. Closing the active
// change clears the local pointer.
func (s Store) CloseChange(root, name, reason string) (Change, error) {
	changes, err := s.ReadChanges(root)
	if err != nil {
		return Change{}, err
	}
	c := s.resolve(root, name, changes)
	if err := s.checkFolder(root, c); err != nil {
		return Change{}, err
	}
	l, err := s.loopOf(c)
	if err != nil {
		return Change{}, err
	}
	if err := l.Machine.Close(&c, reason); err != nil {
		return Change{}, err
	}
	if err := s.CheckStateFiles(root); err != nil {
		return Change{}, err
	}
	if err := s.write(root, &c); err != nil {
		return Change{}, err
	}
	active, err := workspace.ReadActive(root, s.tool())
	if err != nil {
		return Change{}, err
	}
	if active == name {
		if err := os.Remove(workspace.ActivePath(root, s.tool())); err != nil {
			return Change{}, err
		}
	}
	return c, s.Record(root, []string{name + ": closed (" + reason + ")"})
}

// Log appends one timestamped line to LOG.md without committing.
func (s Store) Log(root, line string) error {
	if err := workspace.CheckPath(root, s.LogPath(root)); err != nil {
		return err
	}
	return changelog.Append(s.LogPath(root), s.now(), line)
}

// Record logs lines and, when Commit is on and VCS is not none, commits them with
// STATE.json and any extra paths, relative to root. The first line becomes the
// commit subject.
func (s Store) Record(root string, lines []string, paths ...string) error {
	if err := s.checkVCS(); err != nil {
		return err
	}
	for _, line := range lines {
		if err := s.Log(root, line); err != nil {
			return err
		}
	}
	if !s.commits() {
		return nil
	}
	return s.commit(root, s.tool()+": "+lines[0], append([]string{"STATE.json", "LOG.md"}, paths...))
}

// commit commits exactly paths. Anything else already staged stays staged and out
// of this commit. Outside a git repo the transition stands and this only warns;
// a failed commit is an error, because the state on disk is now ahead of git.
func (s Store) commit(root, message string, paths []string) error {
	if _, err := s.git(root, "rev-parse", "--is-inside-work-tree"); err != nil {
		fmt.Fprintf(s.warn(), "warning: %s is not in a git repo — this state change was not committed; set \"vcs\": \"none\" if this workspace is meant to be local\n", root)
		return nil
	}
	failed := func(err error) error {
		return fmt.Errorf("%s: state saved but not committed: %v", s.tool(), err)
	}
	args := append([]string{"add", "--"}, paths...)
	if _, err := s.git(root, args...); err != nil {
		return failed(err)
	}
	// `git commit --only` refuses a path git knows nothing about, such as an empty
	// folder, so keep only the paths that have something to commit.
	var changed []string
	for _, p := range paths {
		if _, err := s.git(root, "diff", "--cached", "--quiet", "--", p); err != nil {
			changed = append(changed, p)
		}
	}
	if len(changed) == 0 {
		return nil
	}
	args = append([]string{"commit", "--quiet", "--only", "-m", message, "--"}, changed...)
	if _, err := s.git(root, args...); err != nil {
		return failed(err)
	}
	return nil
}

func (s Store) git(root string, args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = root
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %v: %s", args[0], err, strings.TrimSpace(string(out)))
	}
	return string(out), nil
}
