package lock

import (
	"strings"
	"testing"

	core "github.com/btdstudio/daiku/internal/core/state"
)

func build(name, repo, phase, status string) *core.Change {
	return &core.Change{Name: name, Loop: core.BuildLoopName, Phase: phase, Status: status, Repos: []string{repo}}
}

// Spec R29 / D6: who holds a repo.
func TestHolder(t *testing.T) {
	me := *build("N-0040", "api", "tasks", "approved")
	for _, tc := range []struct {
		name   string
		other  *core.Change
		holder string
	}{
		{"implement", build("S-0036", "api", "implement", "in-progress"), "S-0036"},
		{"implement complete", build("S-0036", "api", "implement", "complete"), "S-0036"},
		{"test", build("S-0036", "api", "test", "await-approval"), "S-0036"},
		{"review", build("S-0036", "api", "review", "approved"), "S-0036"},
		{"security skipped", build("S-0036", "api", "security", "skipped"), "S-0036"},
		{"ship in progress", build("S-0036", "api", "ship", "in-progress"), "S-0036"},
		{"ship complete", build("S-0036", "api", "ship", "complete"), ""},
		{"reopened to tasks", build("S-0036", "api", "tasks", "draft"), ""},
		{"before implement", build("S-0036", "api", "design", "approved"), ""},
		{"closed", build("S-0036", "api", "implement", core.Closed), ""},
		{"another repo", build("S-0036", "web", "implement", "in-progress"), ""},
		{"a plan change", &core.Change{Name: "7-gateway", Loop: core.PlanLoopName, Phase: "publish", Status: "in-progress", Repos: []string{"api"}}, ""},
	} {
		t.Run(tc.name, func(t *testing.T) {
			changes := map[string]*core.Change{me.Name: &me, tc.other.Name: tc.other}
			if got, _ := Holder(changes, me); got != tc.holder {
				t.Fatalf("holder = %q, want %q", got, tc.holder)
			}
		})
	}
}

func TestHolderIgnoresItselfAndPicksTheFirstName(t *testing.T) {
	me := build("S-0036", "api", "implement", "in-progress")
	changes := map[string]*core.Change{"S-0036": me}
	if got, _ := Holder(changes, *me); got != "" {
		t.Fatalf("a build held its own repo against itself: %q", got)
	}
	changes["X-9"] = build("X-9", "api", "test", "approved")
	changes["N-0002"] = build("N-0002", "api", "review", "approved")
	next := *build("N-0040", "api", "tasks", "approved")
	if got, at := Holder(changes, next); got != "N-0002" || at.Phase != "review" {
		t.Fatalf("holder = %q (%s)", got, at.Phase)
	}
	if got, _ := Holder(changes, core.Change{Name: "7-gateway", Loop: core.PlanLoopName, Repos: []string{"api"}}); got != "" {
		t.Fatalf("a plan change was locked out: %q", got)
	}
}

func TestMessage(t *testing.T) {
	got := Message("api", "S-0036", *build("S-0036", "api", "implement", "in-progress"))
	if !strings.Contains(got, "api is locked by S-0036 (implement in-progress)") || !strings.Contains(got, "reopen it to tasks") {
		t.Fatalf("message = %q", got)
	}
}
