package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/state"
)

// The overview's build section (S-0042 R7–R9, D1–D5). The plan is 1-auth-flow,
// with ready ticket S-0001 for app, and its build S-0001 is active.

// plannedBuild is plan 1-auth-flow through breakdown, with build S-0001 of its
// ticket active. Under git the workspace is a repo, so transitions commit.
func plannedBuild(t *testing.T, withGit bool) string {
	t.Helper()
	settingsJSON := `{"vcs": "none"}`
	if withGit {
		settingsJSON = ""
	}
	root := vcsWorkspace(t, settingsJSON)
	if withGit {
		gitRepo(t, root)
	}
	if code := runChange(root, []string{"new", "Auth flow"}); code != 0 {
		t.Fatal("change new")
	}
	ticketRepo(t, root)
	writeArtifacts(t, root)
	if code := runTickets(root, []string{"new", "story", "Login", "--repo", "app", "--requirements", "R1"}); code != 0 {
		t.Fatal("tickets new")
	}
	fillCriteria(t, root, "S-0001")
	approveThrough(t, root, "breakdown")
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"ready", "S-0001"}) }); code != 0 {
		t.Fatalf("ready = %d: %s", code, out)
	}
	if code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", "S-0001"}) }); code != 0 {
		t.Fatalf("build new = %d: %s", code, stderr)
	}
	return root
}

// buildsRow is S-0001's row in the README's Builds section.
func buildsRow(t *testing.T, root string) string {
	t.Helper()
	readme := readFile(t, filepath.Join(root, filepath.FromSlash(authReadme)))
	i := strings.Index(readme, "\n## Builds\n")
	if i < 0 {
		t.Fatalf("README has no Builds section:\n%s", readme)
	}
	for _, l := range strings.Split(readme[i:], "\n") {
		if strings.HasPrefix(l, "| [S-0001]") {
			return l
		}
	}
	t.Fatalf("Builds has no S-0001 row:\n%s", readme[i:])
	return ""
}

// narrative is the README outside the generated region.
func narrative(t *testing.T, root string) string {
	t.Helper()
	readme := readFile(t, filepath.Join(root, filepath.FromSlash(authReadme)))
	start, end := strings.Index(readme, "<!-- daiku:generated"), strings.Index(readme, "<!-- /daiku:generated")
	if start < 0 || end < 0 {
		t.Fatalf("README markers:\n%s", readme)
	}
	return readme[:start] + readme[end:]
}

// AC3 and AC7: a build's gate, reopen and close each rewrite its plan's Builds
// row, and leave everything outside the generated region as it was.
func TestBuildTransitionsRefreshThePlansOverview(t *testing.T) {
	root := plannedBuild(t, false)
	readmePath := filepath.Join(root, filepath.FromSlash(authReadme))
	readme := readFile(t, readmePath)
	readme = strings.Replace(readme, "<!-- /daiku:narrative", "Hand-written narrative.\n<!-- /daiku:narrative", 1)
	if err := os.WriteFile(readmePath, []byte(readme), 0o644); err != nil {
		t.Fatal(err)
	}
	before := narrative(t, root)

	gateAll(t, root, throughTasks...)
	gateAll(t, root, []string{"implement", "in-progress"}, []string{"implement", "complete"}, []string{"test", "await-approval"})
	if got, want := buildsRow(t, root), "| [S-0001](tickets/S-0001.md) | `app` | test await-approval |"; got != want {
		t.Fatalf("after gate: %s\nwant: %s", got, want)
	}
	if code, stderr := captureStderr(t, func() int { return runReopen(root, []string{"implement", "--reason", "fails AC1"}) }); code != 0 {
		t.Fatalf("reopen = %d: %s", code, stderr)
	}
	if got := buildsRow(t, root); !strings.Contains(got, "| implement ") {
		t.Fatalf("after reopen: %s", got)
	}
	if code, stderr := captureStderr(t, func() int { return runChange(root, []string{"close", "S-0001", "--reason", "dropped"}) }); code != 0 {
		t.Fatalf("close = %d: %s", code, stderr)
	}
	if got, want := buildsRow(t, root), "| [S-0001](tickets/S-0001.md) | `app` | closed |"; got != want {
		t.Fatalf("after close: %s\nwant: %s", got, want)
	}
	if after := narrative(t, root); after != before {
		t.Fatalf("outside the generated region changed:\n%s\nwas:\n%s", after, before)
	}
	current(t, root, "after close")
}

// N5: under git, the plan's README goes into the build transition's commit.
func TestBuildTransitionCommitsThePlansOverview(t *testing.T) {
	root := plannedBuild(t, true)
	gateAll(t, root, []string{"ticket", "approved"})
	inHead(t, root, "build gate", authReadme)
	if got := buildsRow(t, root); !strings.Contains(got, "| ticket approved |") {
		t.Fatalf("row = %s", got)
	}
	if status := git(t, root, "status", "--porcelain", "--", "plan/specs/1-auth-flow"); strings.TrimSpace(status) != "" {
		t.Fatalf("the plan's folder left uncommitted:\n%s", status)
	}
}

// AC4: in a build-only workspace, a ticket whose change names an absent plan
// gets no overview, and its gate succeeds.
func TestBuildWithAnAbsentPlanWritesNoOverview(t *testing.T) {
	root := buildRoot(t)
	tk, err := ticket.New(ticket.Ticket{ID: "X-1234", Type: ticket.Task, Title: "Fix login", Repo: "api", Change: "7-gateway", Body: "\n"})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "build", "tickets"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "build", "tickets", "X-1234.md"), tk.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := state.SeedBuild(root, "X-1234", "api", "build/tickets/X-1234.md"); err != nil {
		t.Fatal(err)
	}
	code, stderr := gateStderr(t, root, "ticket", "approved")
	if code != 0 || stderr != "" {
		t.Fatalf("gate = %d: %q", code, stderr)
	}
	if isDir(filepath.Join(root, "plan")) {
		t.Fatal("the gate created plan/")
	}
}
