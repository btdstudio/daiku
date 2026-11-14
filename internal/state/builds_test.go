package state

import (
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/plan/overview"
)

// writeTicket writes a ticket at rel (relative to root).
func writeTicket(t *testing.T, root, rel string, tk ticket.Ticket) {
	t.Helper()
	tk.Body = "\n"
	out, err := ticket.New(tk)
	if err != nil {
		t.Fatal(err)
	}
	full := filepath.Join(root, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(full, out.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}
}

// seedBuildOf writes a ticket at rel and records its build.
func seedBuildOf(t *testing.T, root, rel string, tk ticket.Ticket) State {
	t.Helper()
	writeTicket(t, root, rel, tk)
	c, err := SeedBuild(root, tk.ID, tk.Repo, rel)
	if err != nil {
		t.Fatal(err)
	}
	return c
}

// S-0042 D1: the folder names the plan, then the change field, and only a plan
// that's here counts.
func TestPlanOf(t *testing.T) {
	root := overviewRoot(t, "")
	planned := seedBuildOf(t, root, "plan/specs/1-auth/tickets/S-0001.md",
		ticket.Ticket{ID: "S-0001", Type: ticket.Story, Title: "Sign in", Repo: "api", Change: "2-other"})
	inbox := seedBuildOf(t, root, "build/tickets/X-12.md",
		ticket.Ticket{ID: "X-12", Type: ticket.Task, Title: "Fix", Repo: "api", Change: "2-other"})
	absent := seedBuildOf(t, root, "build/tickets/X-13.md",
		ticket.Ticket{ID: "X-13", Type: ticket.Task, Title: "Fix", Repo: "api", Change: "7-gateway"})
	none := seedBuildOf(t, root, "build/tickets/N-0001.md",
		ticket.Ticket{ID: "N-0001", Type: ticket.Task, Title: "Fix", Repo: "api"})
	toBuild := seedBuildOf(t, root, "build/tickets/X-14.md",
		ticket.Ticket{ID: "X-14", Type: ticket.Task, Title: "Fix", Repo: "api", Change: "X-12"})
	for _, tc := range []struct {
		name string
		c    State
		want string
	}{
		{"folder wins over the field", planned, "1-auth"},
		{"inbox ticket's change", inbox, "2-other"},
		{"absent plan", absent, ""},
		{"no change", none, ""},
		{"change names a build", toBuild, ""},
		{"a plan change", State{Name: "1-auth", Loop: PlanLoop}, ""},
	} {
		if got := PlanOf(root, tc.c); got != tc.want {
			t.Errorf("%s: PlanOf = %q, want %q", tc.name, got, tc.want)
		}
	}
}

// D2: the plan's tickets with a repo, plus inbox builds of the plan, by id.
func TestBuildRows(t *testing.T) {
	root := overviewRoot(t, "")
	writeTicket(t, root, "plan/specs/1-auth/tickets/E-0001.md", ticket.Ticket{ID: "E-0001", Type: ticket.Epic, Title: "Accounts"})
	writeTicket(t, root, "plan/specs/1-auth/tickets/S-0003.md", ticket.Ticket{ID: "S-0003", Type: ticket.Story, Title: "Sign out", Repo: "web"})
	seedBuildOf(t, root, "plan/specs/1-auth/tickets/S-0002.md", ticket.Ticket{ID: "S-0002", Type: ticket.Story, Title: "Sign in", Repo: "api"})
	seedBuildOf(t, root, "build/tickets/X-12.md", ticket.Ticket{ID: "X-12", Type: ticket.Task, Title: "Fix", Repo: "api", Change: "1-auth"})
	seedBuildOf(t, root, "build/tickets/X-13.md", ticket.Ticket{ID: "X-13", Type: ticket.Task, Title: "Other", Repo: "api", Change: "2-other"})
	if _, err := CloseChange(root, "X-12", "dropped"); err != nil {
		t.Fatal(err)
	}
	rows, err := BuildRows(root, "1-auth")
	if err != nil {
		t.Fatal(err)
	}
	want := []overview.BuildRow{
		{ID: "S-0002", Repo: "api", Phase: "ticket", Status: "draft", Started: true},
		{ID: "S-0003", Repo: "web"},
		{ID: "X-12", Repo: "api", Phase: "ticket", Status: "closed", Started: true, Closed: true},
	}
	if !reflect.DeepEqual(rows, want) {
		t.Fatalf("rows = %+v\nwant %+v", rows, want)
	}
}

// D5: CheckOverview refuses a symlinked README, and does nothing under manual
// or without a plan.
func TestCheckOverview(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks")
	}
	for _, mode := range []string{"", "manual"} {
		root := overviewRoot(t, mode)
		build := seedBuildOf(t, root, "plan/specs/1-auth/tickets/S-0002.md", ticket.Ticket{ID: "S-0002", Type: ticket.Story, Title: "Sign in", Repo: "api"})
		lone := seedBuildOf(t, root, "build/tickets/N-0001.md", ticket.Ticket{ID: "N-0001", Type: ticket.Task, Title: "Fix", Repo: "web"})
		readme := filepath.Join(root, filepath.FromSlash(OverviewPath("1-auth")))
		os.Remove(readme)
		if err := os.Symlink(filepath.Join(t.TempDir(), "x.md"), readme); err != nil {
			t.Fatal(err)
		}
		plan := State{Name: "1-auth", Loop: PlanLoop}
		for _, c := range []State{build, plan} {
			err := CheckOverview(root, c)
			if mode == "manual" && err != nil {
				t.Errorf("manual, %s: %v", c.Name, err)
			}
			if mode == "" && (err == nil || !strings.Contains(err.Error(), "is a symlink")) {
				t.Errorf("auto, %s: err = %v", c.Name, err)
			}
		}
		if err := CheckOverview(root, lone); err != nil {
			t.Errorf("%q, no plan: %v", mode, err)
		}
	}
}
