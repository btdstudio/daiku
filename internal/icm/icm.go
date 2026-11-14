// Package icm configures the core scaffold engine for daiku: the embedded root
// tree, the rooms, the generated seeds, and AGENTS.md, which is assembled from
// fragments rather than copied.
package icm

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"

	"github.com/btdstudio/daiku"
	"github.com/btdstudio/daiku/internal/agents"
	"github.com/btdstudio/daiku/internal/core/scaffold"
	"github.com/btdstudio/daiku/internal/core/workspace"
	"github.com/btdstudio/daiku/internal/settings"
)

// WorkspaceDir is the per-project config directory. It is committed, except for
// the local .daiku/active and .daiku/paths.json.
const WorkspaceDir = ".daiku"

// PlanRoom is the plan room's folder.
const PlanRoom = "plan"

// BuildRoom is the build room's folder.
const BuildRoom = "build"

type (
	Manifest = scaffold.Manifest
	Options  = scaffold.Options
)

// Report is the engine's init report, plus an existing AGENTS.md init couldn't
// merge into, and why. That file is left as it was.
type Report struct {
	scaffold.Report
	NotMerged map[string]string
}

// UpdateReport is the engine's report, plus what daiku did to AGENTS.md.
type UpdateReport struct {
	scaffold.UpdateReport
	Merged    []string          // files whose daiku-owned sections were merged in
	NotMerged map[string]string // files daiku couldn't merge, and why
}

var engine = scaffold.Engine{
	Tool:  "daiku",
	FS:    daiku.RootFS,
	Rooms: map[string]fs.FS{PlanRoom: daiku.PlanFS, BuildRoom: daiku.BuildFS},
	// build/tickets/ (the inbox) can exist before the build room is first used, so
	// the room is there once its 01_design/ is.
	RoomMarkers: map[string]string{BuildRoom: "01_design"},
	Marker:      WorkspaceDir,
	Seeds:       seeds,
}

// seeds generates settings, STATE.json, LOG.md and REPOS.json. They are
// produced by the binary — not copied from the scaffold — so `daiku update`
// never touches the workspace's own state.
func seeds() ([]scaffold.Seed, error) {
	settingsData, err := json.MarshalIndent(settings.DefaultSettings(), "", "  ")
	if err != nil {
		return nil, err
	}
	stateData, err := json.MarshalIndent(map[string]any{
		"schema":  1,
		"changes": map[string]any{},
	}, "", "  ")
	if err != nil {
		return nil, err
	}
	reposData, err := json.MarshalIndent(map[string]any{"repos": map[string]any{}}, "", "  ")
	if err != nil {
		return nil, err
	}
	return []scaffold.Seed{
		{Rel: WorkspaceDir + "/settings.json", Data: append(settingsData, '\n')},
		{Rel: "STATE.json", Data: append(stateData, '\n')},
		{Rel: "LOG.md", Data: []byte{}},
		{Rel: "REPOS.json", Data: append(reposData, '\n')},
	}, nil
}

// Scaffold writes the root tree into dir, then AGENTS.md; see
// scaffold.Engine.Scaffold. It writes no room: a room appears the first time
// it's used. vcs is the new workspace's "vcs" setting ("" means git); under
// "none" there's no git init and no git files, and the project settings record
// "vcs": "none". A zumen or tomo workspace is refused before anything is
// written. An existing AGENTS.md that can't be merged is left alone and
// reported in NotMerged, as `update` does, rather than failing init.
func Scaffold(dir string, opts Options, vcs string) (Report, error) {
	if err := workspace.CheckLegacy(dir); err != nil {
		return Report{}, err
	}
	e := engine
	e.VCS = vcs
	base, err := e.Scaffold(dir, opts)
	rep := Report{Report: base}
	if err != nil {
		return rep, err
	}
	written, _, err := WriteAgents(dir, opts.Force)
	var unmergeable errUnmergeable
	switch {
	case errors.As(err, &unmergeable):
		rep.NotMerged = map[string]string{agentsFile: unmergeable.reason}
	case err != nil:
		return rep, err
	case written:
		rep.Written = append(rep.Written, agentsFile)
	}
	return rep, nil
}

// Update refreshes managed files for the root and every room the workspace has,
// under the workspace's "vcs" setting; see scaffold.Engine.Update. Then
// AGENTS.md is re-assembled and merged into the workspace's copy, keeping the
// person's own sections.
func Update(dir string) (UpdateReport, error) {
	if err := workspace.CheckLegacy(dir); err != nil {
		return UpdateReport{}, err
	}
	loaded, err := settings.LoadSettings(dir)
	if err != nil {
		return UpdateReport{}, err
	}
	e := engine
	e.VCS = loaded.VCS
	base, err := e.Update(dir)
	rep := UpdateReport{UpdateReport: base}
	if err != nil {
		return rep, err
	}
	return rep, mergeAgentsFile(dir, &rep)
}

// EnsureBuild adds the build room the first time it's needed and merges its
// AGENTS.md sections in. It reports whether it created the room; once the room
// is there it does nothing. An inbox folder alone doesn't count as the room.
func EnsureBuild(dir string) (created bool, err error) { return ensureRoom(dir, BuildRoom) }

// EnsurePlan adds the plan room the first time it's needed and merges its
// AGENTS.md sections in. It reports whether it created the room; once plan/
// exists it does nothing.
func EnsurePlan(dir string) (created bool, err error) { return ensureRoom(dir, PlanRoom) }

// ensureRoom adds room through the engine and merges its AGENTS.md sections in.
func ensureRoom(dir, room string) (created bool, err error) {
	loaded, err := settings.LoadSettings(dir)
	if err != nil {
		return false, err
	}
	e := engine
	e.VCS = loaded.VCS
	rep, err := e.EnsureRoom(dir, room)
	if err != nil || !rep.Created {
		return false, err
	}
	if _, _, err := WriteAgents(dir, false); err != nil {
		return true, err
	}
	return true, nil
}

const agentsFile = "AGENTS.md"

// rooms lists the rooms present in dir, for AGENTS.md assembly.
func rooms(dir string) []string {
	var out []string
	for _, r := range agents.Rooms {
		if engine.HasRoom(dir, r) {
			out = append(out, r)
		}
	}
	return out
}

// errUnmergeable reports why a person's AGENTS.md couldn't be merged.
type errUnmergeable struct{ reason string }

func (e errUnmergeable) Error() string { return "daiku: can't merge " + agentsFile + ": " + e.reason }

// WriteAgents assembles AGENTS.md from the root fragment and the fragments of
// the rooms dir has. A missing file (or force) gets the assembled text; an
// existing one gets it merged in by section, so the person's sections survive.
// written reports a new or replaced file, merged a changed merge. A file that
// can't be merged is left alone and the reason returned as an errUnmergeable.
func WriteAgents(dir string, force bool) (written, merged bool, err error) {
	if link := workspace.ThroughSymlink(dir, agentsFile); link != "" {
		return false, false, fmt.Errorf("daiku: won't write %s: %s is a symlink, which could lead outside the workspace", agentsFile, link)
	}
	assembled, err := agents.Assemble(rooms(dir)...)
	if err != nil {
		return false, false, err
	}
	path := filepath.Join(dir, agentsFile)
	user, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) || (err == nil && force) {
		return true, false, os.WriteFile(path, []byte(assembled), 0o644)
	}
	if err != nil {
		return false, false, err
	}
	out, reason := agents.Merge(string(user), assembled)
	if reason != "" {
		return false, false, errUnmergeable{reason}
	}
	if out == string(user) {
		return false, false, nil
	}
	// The file exists, so WriteFile keeps its mode.
	return false, true, os.WriteFile(path, []byte(out), 0o644)
}

// mergeAgentsFile runs WriteAgents for Update and records the outcome in rep: a
// merge in Merged, an unmergeable file in Skipped and NotMerged, no change in
// Unchanged, a missing file restored in Updated.
func mergeAgentsFile(dir string, rep *UpdateReport) error {
	written, merged, err := WriteAgents(dir, false)
	var unmergeable errUnmergeable
	switch {
	case errors.As(err, &unmergeable):
		if rep.NotMerged == nil {
			rep.NotMerged = map[string]string{}
		}
		rep.NotMerged[agentsFile] = unmergeable.reason
		rep.Skipped = append(rep.Skipped, agentsFile)
		return nil
	case err != nil:
		return err
	case written:
		rep.Updated = append(rep.Updated, agentsFile)
	case merged:
		rep.Merged = append(rep.Merged, agentsFile)
	default:
		rep.Unchanged = append(rep.Unchanged, agentsFile)
	}
	return nil
}

// Check validates the embedded scaffold: scaffold.Engine.Check, then the docs
// lint, which keeps each room's docs to its own loop's terms (lint.go).
func Check() []string { return append(engine.Check(), lintDocs(scaffoldRooms())...) }

// BuildManifest hashes the embedded trees present in dir.
func BuildManifest(dir string) (Manifest, error) { return engine.BuildManifest(dir) }

// ReadManifest reads .daiku/manifest.json.
func ReadManifest(dir string) (Manifest, error) { return engine.ReadManifest(dir) }

// WriteManifest writes .daiku/manifest.json.
func WriteManifest(dir string, m Manifest) error { return engine.WriteManifest(dir, m) }
