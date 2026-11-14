// Package state configures the core phase machine for daiku: one store holding
// every change, each following its own loop.
//
// The plan loop is intake → research → explore → synthesis → spec →
// architecture → breakdown → publish. Research, explore and synthesis are
// skippable, and publish is a run phase. The build loop is ticket →
// requirements → design → tasks → implement → test → review → security → ship;
// its behaviour arrives in a later version, but it's registered so a STATE.json
// holding build changes stays readable.
//
// Layout:
//
//	STATE.json         every change's record: { schema, changes } maps each change
//	                   to its { loop, phase, status, gates, sources, repos,
//	                   updatedAt }, committed so approvals survive a clone.
//	LOG.md             one timestamped line per state change / gate (committed)
//	.daiku/active      the active change's name; gitignored, because each
//	                   checkout works on its own change
//	REPOS.json         logical repos (committed); .daiku/paths.json is local
//	plan/specs/        per plan change: <id>-<slug>/functional-spec.md,
//	                   architecture.md, tickets/<ID>.md
package state

import (
	"path/filepath"
	"time"

	core "github.com/btdstudio/daiku/internal/core/state"
	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/core/workspace"
	"github.com/btdstudio/daiku/internal/plan"
	"github.com/btdstudio/daiku/internal/plan/layout"
	"github.com/btdstudio/daiku/internal/settings"
)

// PlanMachine is the plan loop. Check is plan.CheckArtifacts: it's set here,
// in the literal, because the store copies the machine by value. The core's
// Advance never runs it; `daiku gate` does, through CheckArtifacts, and applies
// the gates policy to what it reports.
var PlanMachine = core.Machine{Tool: "daiku", Check: plan.CheckArtifacts, Phases: []core.Phase{
	{Name: "intake", Kind: core.Gated},
	{Name: "research", Kind: core.Gated, Skippable: true},
	{Name: "explore", Kind: core.Gated, Skippable: true},
	{Name: "synthesis", Kind: core.Gated, Skippable: true},
	{Name: "spec", Kind: core.Gated},
	{Name: "architecture", Kind: core.Gated},
	{Name: "breakdown", Kind: core.Gated},
	{Name: "publish", Kind: core.Run},
}}

// BuildMachine is the build loop's phases (spec R12). It has no artifact checks;
// `daiku gate` runs the repo lock on a build entering implement.
var BuildMachine = core.Machine{Tool: "daiku", Phases: []core.Phase{
	{Name: "ticket", Kind: core.Gated},
	{Name: "requirements", Kind: core.Gated},
	{Name: "design", Kind: core.Gated},
	{Name: "tasks", Kind: core.Gated},
	{Name: "implement", Kind: core.Run},
	{Name: "test", Kind: core.Gated},
	{Name: "review", Kind: core.Gated},
	{Name: "security", Kind: core.Gated, Skippable: true},
	{Name: "ship", Kind: core.Run},
}}

// The loops a change can follow, as STATE.json records them.
const (
	PlanLoop  = core.PlanLoopName
	BuildLoop = core.BuildLoopName
)

// Reasons for the skips `daiku change new` records.
const (
	NoRepoReason     = "no repo in the intake"
	SingleRepoReason = "fewer than two repos in scope"
)

// Now is the injectable clock used for timestamps (updatedAt and log lines).
// Tests override it to make output deterministic.
var Now = time.Now

var store = core.Store{
	Tool: "daiku",
	Loops: map[string]core.Loop{
		PlanLoop:  planLoop(),
		BuildLoop: core.BuildLoop(BuildMachine, ticket.IDRe),
	},
	Commit: true,
	Now:    func() time.Time { return Now() },
}

// planLoop is the core plan layout with the plan machine. Gates commit each
// change's research notes and findings too.
func planLoop() core.Loop {
	l := core.PlanLoop(PlanMachine)
	l.ChangeDirs = []string{layout.ResearchDir, layout.ExploreDir}
	return l
}

// storeAt is the store for the workspace at root, carrying its "vcs" setting so
// a local workspace (vcs: none) never runs git. Only the wrappers that record a
// transition need it; paths and reads use store directly.
func storeAt(root string) (core.Store, error) {
	loaded, err := settings.LoadSettings(root)
	if err != nil {
		return core.Store{}, err
	}
	s := store
	s.VCS = loaded.VCS
	return s, nil
}

type (
	// State is one change's machine.
	State = core.Change
	// KnownChange is one change annotated with any recorded progress.
	KnownChange = core.KnownChange
)

func Workspace(root string) string  { return workspace.Dir(root, "daiku") }
func StatePath(root string) string  { return store.StatePath(root) }
func ActivePath(root string) string { return workspace.ActivePath(root, "daiku") }
func SpecsDir(root string) string   { return filepath.Join(root, filepath.FromSlash(layout.SpecsDir)) }
func LogPath(root string) string    { return store.LogPath(root) }

// ChangeDir returns the path to one plan change's folder. An empty change
// resolves to the specs base itself (a guard for ad-hoc outputs when nothing is
// active).
func ChangeDir(root, change string) string {
	if change == "" {
		return SpecsDir(root)
	}
	return filepath.Join(SpecsDir(root), change)
}

// ArtifactPath is an artifact inside a specific change folder.
func ArtifactPath(root, change, name string) string {
	return filepath.Join(ChangeDir(root, change), name)
}

// NextChangeId is the next sequential change id: past every change folder and
// every change STATE.json records, so a closed change's id is never reused.
func NextChangeId(root string) (int, error) { return store.NextID(root, PlanLoop) }

// Slugify lowercases to a short kebab-case slug.
func Slugify(text string) string { return core.Slugify(text) }

// DefaultState is the machine when nothing is recorded: a plan change, since
// that's the only loop with commands so far.
func DefaultState() State {
	c := core.NewChange("")
	c.Loop = PlanLoop
	return c
}

// ReadActive returns the active change's name ("" when none is set).
func ReadActive(root string) (string, error) { return workspace.ReadActive(root, "daiku") }

// LoadState resolves the active machine. With no active change and no record,
// it's DefaultState.
func LoadState(root string) (State, error) {
	st, err := store.Load(root)
	if err == nil && st.Name == "" && st.Loop == "" {
		return DefaultState(), nil
	}
	return st, err
}

// SaveState writes the change's record and makes it the active change.
func SaveState(root string, st *State) error { return store.Save(root, st) }

// ReadChanges returns the recorded changes.
func ReadChanges(root string) (map[string]*State, error) { return store.ReadChanges(root) }

// ChangeGates returns a copy of the named change's gates. A change with no record,
// or an empty name, has no gates yet, so it gets an empty map.
func ChangeGates(root, name string) (map[string]string, error) {
	changes, err := ReadChanges(root)
	if err != nil {
		return nil, err
	}
	gates := map[string]string{}
	if c, ok := changes[name]; ok && c != nil {
		for phase, status := range c.Gates {
			gates[phase] = status
		}
	}
	return gates, nil
}

// KnownChanges lists every loop's changes: the plan loop's numbered folders
// under plan/specs/ sorted by id, then the build loop's recorded changes.
func KnownChanges(root string) ([]KnownChange, error) { return store.Known(root) }

// PlanChanges is KnownChanges limited to the plan loop: the changes that have
// a folder under plan/specs/, tickets and an overview.
func PlanChanges(root string) ([]KnownChange, error) {
	all, err := KnownChanges(root)
	if err != nil {
		return nil, err
	}
	var plan []KnownChange
	for _, c := range all {
		if c.Loop == PlanLoop {
			plan = append(plan, c)
		}
	}
	return plan, nil
}

// SwitchTo points the workspace at a different change folder.
func SwitchTo(root, name string) (State, error) { return store.Switch(root, name) }

// MachineFor is the machine a loop runs: the build machine for build, and the
// plan machine otherwise, so a change recorded without a loop is a plan change.
func MachineFor(loop string) core.Machine {
	if loop == BuildLoop {
		return BuildMachine
	}
	return PlanMachine
}

// SetGate starts the given phase and sets its status, in the change's own loop.
func SetGate(st *State, phase, status, reason string) error {
	return MachineFor(st.Loop).SetGate(st, phase, status, reason)
}

// CheckGate reports whether setting phase to status respects the change's loop.
func CheckGate(st State, phase, status string) error {
	return MachineFor(st.Loop).CheckGate(st, phase, status)
}

// SeedChange creates a plan change folder and records its initial machine, making it
// the active change. Without a repo there's nothing to explore, and with fewer than
// two there's nothing to synthesize, so those start skipped. Under vcs: git,
// sources are committed with it, so a clone has what the record points at.
func SeedChange(root, name string, hasRepo bool, sources []string, repos ...string) (State, error) {
	opts := core.SeedOptions{Sources: sources, Repos: repos, Paths: sources}
	opts.Skip = map[string]string{}
	if !hasRepo {
		opts.Skip["explore"] = NoRepoReason
	}
	if len(repos) < 2 {
		opts.Skip["synthesis"] = SingleRepoReason
	}
	s, err := storeAt(root)
	if err != nil {
		return State{}, err
	}
	return s.Seed(root, name, PlanLoop, opts)
}

// SeedBuild creates a build change named by its ticket id, in build/<repo>/changes/<id>/,
// and makes it active. Its one source is the ticket's path, relative to root: the
// ticket is read in place, never copied. Under vcs: git the ticket is committed
// with the new change, so a clone has what the record points at.
func SeedBuild(root, id, repo, source string) (State, error) {
	s, err := storeAt(root)
	if err != nil {
		return State{}, err
	}
	return s.Seed(root, id, BuildLoop, core.SeedOptions{Sources: []string{source}, Repos: []string{repo}, Paths: []string{source}})
}

// AdvanceGate sets a gate on the active change, logs it with any notes, and
// commits under vcs: git.
func AdvanceGate(root, phase, status, reason string, notes ...string) (State, error) {
	s, err := storeAt(root)
	if err != nil {
		return State{}, err
	}
	return s.Advance(root, phase, status, reason, notes...)
}

// Reopen moves the active change back to a phase, logs it and commits under
// vcs: git.
func Reopen(root, phase, reason string) (State, error) {
	s, err := storeAt(root)
	if err != nil {
		return State{}, err
	}
	return s.Reopen(root, phase, reason)
}

// CloseChange closes a change by name, logs it and commits under vcs: git.
func CloseChange(root, name, reason string) (State, error) {
	s, err := storeAt(root)
	if err != nil {
		return State{}, err
	}
	return s.CloseChange(root, name, reason)
}

// Record logs lines and, under vcs: git, commits them with the given paths,
// relative to root.
func Record(root string, lines []string, paths ...string) error {
	s, err := storeAt(root)
	if err != nil {
		return err
	}
	return s.Record(root, lines, paths...)
}

// CheckStateFiles refuses when STATE.json, the active pointer or LOG.md can't be
// written safely. A command that writes its own files and then calls Record runs
// it first, so a refusal leaves those files as they were.
func CheckStateFiles(root string) error {
	s, err := storeAt(root)
	if err != nil {
		return err
	}
	return s.CheckStateFiles(root)
}

// AppendLog appends one timestamped line to LOG.md.
func AppendLog(root, line string) error { return store.Log(root, line) }
