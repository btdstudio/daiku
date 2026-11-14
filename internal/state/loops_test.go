package state

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	core "github.com/btdstudio/daiku/internal/core/state"
)

// AC11: a STATE.json holding a plan change and a build change loads, because
// both loops are registered, and each record keeps its loop.
func TestPlanAndBuildRecordsLoad(t *testing.T) {
	root := t.TempDir()
	raw := `{"schema": 1, "changes": {
  "1-gateway": {"loop": "plan", "phase": "intake", "status": "draft", "gates": {"intake": "draft"}, "sources": []},
  "S-0036": {"loop": "build", "phase": "ticket", "status": "draft", "gates": {"ticket": "draft"}, "sources": [], "repos": ["api"]}
}}`
	if err := os.WriteFile(StatePath(root), []byte(raw), 0o644); err != nil {
		t.Fatal(err)
	}
	changes, err := ReadChanges(root)
	if err != nil {
		t.Fatal(err)
	}
	if changes["1-gateway"].Loop != PlanLoop || changes["S-0036"].Loop != BuildLoop {
		t.Fatalf("loops = %q, %q", changes["1-gateway"].Loop, changes["S-0036"].Loop)
	}
	if _, err := KnownChanges(root); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(ChangeDir(root, "1-gateway"), 0o755); err != nil {
		t.Fatal(err)
	}
	plan, err := PlanChanges(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(plan) != 1 || plan[0].Name != "1-gateway" {
		t.Fatalf("PlanChanges = %+v, want only 1-gateway", plan)
	}
}

// N5: no phase name is in both loops, so a gate can't be read as the other
// loop's.
func TestNoPhaseNameInBothLoops(t *testing.T) {
	plan := map[string]bool{}
	for _, p := range PlanMachine.Phases {
		plan[p.Name] = true
	}
	for _, p := range BuildMachine.Phases {
		if plan[p.Name] {
			t.Errorf("phase %q is in both loops", p.Name)
		}
	}
}

// The build machine is spec R12's: nine phases in order, security skippable,
// implement and ship run phases.
func TestBuildMachine(t *testing.T) {
	want := []string{"ticket", "requirements", "design", "tasks", "implement", "test", "review", "security", "ship"}
	if len(BuildMachine.Phases) != len(want) {
		t.Fatalf("phases = %+v", BuildMachine.Phases)
	}
	for i, p := range BuildMachine.Phases {
		if p.Name != want[i] {
			t.Errorf("phase %d = %q, want %q", i, p.Name, want[i])
		}
		run := p.Name == "implement" || p.Name == "ship"
		if (p.Kind == core.Run) != run {
			t.Errorf("%s kind = %v", p.Name, p.Kind)
		}
		if p.Skippable != (p.Name == "security") {
			t.Errorf("%s skippable = %v", p.Name, p.Skippable)
		}
	}
}

// localRoot is an empty workspace with vcs: none, so transitions run no git.
func localRoot(t *testing.T) string {
	t.Helper()
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	root := t.TempDir()
	if err := os.MkdirAll(Workspace(root), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(Workspace(root), "settings.json"), []byte(`{"vcs": "none"}`), 0o644); err != nil {
		t.Fatal(err)
	}
	return root
}

// Spec R16, R17: a build change is named by its ticket id, lives under its repo,
// and records the ticket's path as its source.
func TestSeedBuild(t *testing.T) {
	root := localRoot(t)
	st, err := SeedBuild(root, "N-0001", "api", "build/tickets/N-0001.md")
	if err != nil {
		t.Fatal(err)
	}
	if st.Loop != BuildLoop || st.Phase != "ticket" || st.Status != "draft" {
		t.Fatalf("seeded = %+v", st)
	}
	if info, err := os.Stat(filepath.Join(root, "build", "api", "changes", "N-0001")); err != nil || !info.IsDir() {
		t.Fatalf("change folder: %v", err)
	}
	loaded, err := LoadState(root)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Name != "N-0001" || !reflect.DeepEqual(loaded.Sources, []string{"build/tickets/N-0001.md"}) || !reflect.DeepEqual(loaded.Repos, []string{"api"}) {
		t.Fatalf("loaded = %+v", loaded)
	}
}

// Spec R12: gate checks follow the change's own loop.
func TestGatesFollowTheChangesLoop(t *testing.T) {
	build := State{Name: "N-0001", Loop: BuildLoop, Phase: "ticket", Status: "draft", Gates: map[string]string{"ticket": "draft"}}
	if err := CheckGate(build, "ticket", "approved"); err != nil {
		t.Fatalf("ticket approved: %v", err)
	}
	if err := CheckGate(build, "requirements", "approved"); err == nil {
		t.Fatal("requirements approved before ticket")
	}
	probe := build
	if err := SetGate(&probe, "intake", "approved", ""); err == nil {
		t.Fatal("a build change accepted the plan phase intake")
	}
	plan := State{Name: "1-gateway", Phase: "intake", Status: "draft", Gates: map[string]string{"intake": "draft"}}
	if err := SetGate(&plan, "ticket", "approved", ""); err == nil {
		t.Fatal("a change with no loop accepted the build phase ticket")
	}
	if MachineFor(BuildLoop).Phases[0].Name != "ticket" || MachineFor("").Phases[0].Name != "intake" {
		t.Fatal("MachineFor picked the wrong machine")
	}
}
