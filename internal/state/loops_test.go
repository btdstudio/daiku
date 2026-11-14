package state

import (
	"os"
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
