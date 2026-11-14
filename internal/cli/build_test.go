package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/state"
)

// buildRoot is a local (vcs: none) workspace with repos api and web registered,
// each with a local path, and no changes.
func buildRoot(t *testing.T) string {
	t.Helper()
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	root := newRoot(t)
	if err := os.WriteFile(filepath.Join(root, ".daiku", "settings.json"), []byte(`{"vcs": "none"}`), 0o644); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"api", "web"} {
		if code := repoCmd(root, "add", name, "https://github.com/org/"+name, "--path", filepath.Join(t.TempDir(), name)); code != 0 {
			t.Fatalf("repo add %s = %d", name, code)
		}
	}
	return root
}

// seedBuild records build change id on repo and makes it active. Its source is
// an inbox ticket, written so status can read it.
func seedBuild(t *testing.T, root, id, repo string) {
	t.Helper()
	tk, err := ticket.New(ticket.Ticket{ID: id, Type: ticket.Task, Title: "Fix login", Repo: repo, Body: "\n"})
	if err != nil {
		t.Fatal(err)
	}
	rel := "build/tickets/" + id + ".md"
	if err := os.MkdirAll(filepath.Join(root, "build", "tickets"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, filepath.FromSlash(rel)), tk.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := state.SeedBuild(root, id, repo, rel); err != nil {
		t.Fatal(err)
	}
}

func gateAll(t *testing.T, root string, steps ...[]string) {
	t.Helper()
	for _, c := range steps {
		if code, stderr := gateStderr(t, root, c...); code != 0 {
			t.Fatalf("gate %v = %d: %s", c, code, stderr)
		}
	}
}

var throughTasks = [][]string{{"ticket", "approved"}, {"requirements", "approved"}, {"design", "approved"}, {"tasks", "approved"}}

// AC3, ported from tomo TestGateEnforcesLoopOrder: a build follows its own loop.
func TestBuildGateEnforcesLoopOrder(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "N-0001", "api")
	if code := runGate(root, []string{"design", "approved"}); code != 1 {
		t.Fatalf("out of order = %d, want 1", code)
	}
	gateAll(t, root, append(throughTasks, [][]string{
		{"implement", "in-progress"}, {"implement", "complete"},
		{"test", "approved"}, {"review", "approved"}, {"security", "approved"},
		{"ship", "in-progress"}, {"ship", "complete"},
	}...)...)
	if log := readLog(t, root); strings.Contains(log, "override") {
		t.Fatalf("in-order run logged an override: %s", log)
	}
	if code := runGate(root, []string{"implement", "approved"}); code != 1 {
		t.Fatalf("gated status on run phase = %d, want 1", code)
	}
	if code := runGate(root, []string{"intake", "approved", "--force"}); code != 1 {
		t.Fatalf("a plan phase on a build = %d, want 1", code)
	}
}

// Ported from tomo TestFailedReviewLoopsBackToImplement.
func TestBuildFailedReviewLoopsBackToImplement(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "N-0001", "api")
	gateAll(t, root, append(throughTasks, []string{"implement", "in-progress"})...)
	if code := runGate(root, []string{"test", "approved"}); code != 1 {
		t.Fatalf("test during implement = %d, want 1", code)
	}
	gateAll(t, root, []string{"implement", "complete"}, []string{"test", "approved"}, []string{"review", "approved"},
		[]string{"security", "skipped", "--reason", "docs only"})
	if code := runReopen(root, []string{"implement", "--reason", "critical finding"}); code != 0 {
		t.Fatalf("reopen = %d", code)
	}
	g := gates(t, root)
	if g["implement"] != "in-progress" || g["test"] != "" || g["review"] != "" || g["security"] != "skipped" || g["tasks"] != "approved" {
		t.Fatalf("gates = %+v", g)
	}
	if code := runGate(root, []string{"review", "approved"}); code != 1 {
		t.Fatalf("review before re-testing = %d, want 1", code)
	}
}

// Ported from tomo TestSecurityCanBeSkippedButNotOtherPhases.
func TestBuildSecurityCanBeSkippedButNotOtherPhases(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "N-0001", "api")
	if code := runGate(root, []string{"review", "skipped", "--reason", "r", "--force"}); code != 1 {
		t.Fatalf("skip review = %d, want 1", code)
	}
	if code := runGate(root, []string{"security", "skipped", "--force"}); code != 1 {
		t.Fatalf("skip without reason = %d, want 1", code)
	}
}

// Ported from tomo TestGateForceOverridesAndLogs.
func TestBuildGateForceOverridesAndLogs(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "N-0001", "api")
	if code := runGate(root, []string{"tasks", "approved", "--force"}); code != 0 {
		t.Fatalf("code = %d", code)
	}
	if log := readLog(t, root); !strings.Contains(log, "override: daiku: tasks requires ticket") {
		t.Fatalf("override not logged: %s", log)
	}
}

// Ported from tomo TestReopenAndClose.
func TestBuildReopenAndClose(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "N-0001", "api")
	gateAll(t, root, throughTasks[:3]...)
	if code := runReopen(root, []string{"requirements"}); code != 1 {
		t.Fatalf("reopen without reason = %d, want 1", code)
	}
	if code := runReopen(root, []string{"requirements", "--reason", "scope changed"}); code != 0 {
		t.Fatalf("reopen = %d", code)
	}
	if g := gates(t, root); g["requirements"] != "draft" || g["design"] != "" {
		t.Fatalf("gates = %+v", g)
	}
	if code := runChange(root, []string{"close", "N-0001", "--reason", "dropped"}); code != 0 {
		t.Fatalf("close = %d", code)
	}
	if code := runSwitch(root, []string{"N-0001"}); code != 0 {
		t.Fatalf("switch = %d", code)
	}
	if code := runGate(root, []string{"requirements", "approved", "--force"}); code != 1 {
		t.Fatalf("gate on closed = %d, want 1", code)
	}
}

// AC7, AC11 / spec R29: one build per repo in implement through ship.
func TestRepoLock(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "S-0036", "api")
	gateAll(t, root, append(throughTasks, []string{"implement", "in-progress"})...)
	seedBuild(t, root, "N-0040", "api")
	gateAll(t, root, throughTasks...)
	code, stderr := gateStderr(t, root, "implement", "in-progress")
	if code != 1 || !strings.Contains(stderr, "api is locked by S-0036 (implement in-progress)") {
		t.Fatalf("locked gate = %d: %s", code, stderr)
	}
	if g := gates(t, root); g["implement"] != "" {
		t.Fatalf("a refused gate was recorded: %v", g)
	}
	// Another repo isn't locked.
	seedBuild(t, root, "N-0041", "web")
	gateAll(t, root, append(throughTasks, []string{"implement", "in-progress"})...)

	// Reopening the holder to tasks frees the repo.
	if code := runSwitch(root, []string{"S-0036"}); code != 0 {
		t.Fatal("switch S-0036")
	}
	if code := runReopen(root, []string{"tasks", "--reason", "rethink"}); code != 0 {
		t.Fatal("reopen S-0036")
	}
	if code := runSwitch(root, []string{"N-0040"}); code != 0 {
		t.Fatal("switch N-0040")
	}
	gateAll(t, root, []string{"implement", "in-progress"})

	// --force overrides a held lock, and LOG.md records it.
	if code := runSwitch(root, []string{"S-0036"}); code != 0 {
		t.Fatal("switch S-0036")
	}
	gateAll(t, root, []string{"tasks", "approved"})
	if code, stderr := gateStderr(t, root, "implement", "in-progress"); code != 1 || !strings.Contains(stderr, "locked by N-0040") {
		t.Fatalf("second holder = %d: %s", code, stderr)
	}
	gateAll(t, root, []string{"implement", "in-progress", "--force"})
	if log := readLog(t, root); !strings.Contains(log, "override: daiku: api is locked by N-0040") {
		t.Fatalf("override not logged:\n%s", log)
	}
}

// AC11: a build that has completed ship doesn't hold its repo.
func TestShipCompleteFreesTheRepo(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "S-0036", "api")
	gateAll(t, root, append(throughTasks, [][]string{
		{"implement", "in-progress"}, {"implement", "complete"}, {"test", "approved"}, {"review", "approved"},
		{"security", "skipped", "--reason", "docs only"}, {"ship", "in-progress"}, {"ship", "complete"},
	}...)...)
	seedBuild(t, root, "N-0040", "api")
	gateAll(t, root, append(throughTasks, []string{"implement", "in-progress"})...)
}

// AC9 / spec R21: env prints a plan change's repos, then a build's one repo.
func TestEnvForEachLoop(t *testing.T) {
	root := buildRoot(t)
	if code := runChange(root, []string{"new", "Gateway", "--repo", "api", "--repo", "web"}); code != 0 {
		t.Fatal("change new")
	}
	reg := func(name string) string {
		t.Helper()
		_, out := captureStdout(t, func() int { return repoCmd(root, "list") })
		for _, line := range strings.Split(out, "\n") {
			if f := strings.Fields(line); len(f) >= 3 && f[0] == name {
				return f[len(f)-1]
			}
		}
		t.Fatalf("no path for %s in:\n%s", name, out)
		return ""
	}
	api, web := reg("api"), reg("web")
	for _, p := range []string{api, web} {
		if err := os.MkdirAll(p, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	code, out := captureStdout(t, func() int { return runEnv(root) })
	if code != 0 || strings.TrimSpace(out) != "--add-dir "+api+" --add-dir "+web {
		t.Fatalf("plan env = %d, %q", code, out)
	}
	seedBuild(t, root, "S-0036", "api")
	code, out = captureStdout(t, func() int { return runEnv(root) })
	if code != 0 || strings.TrimSpace(out) != "--add-dir "+api {
		t.Fatalf("build env = %d, %q", code, out)
	}
}

// Spec R15 / D11: status shows a build's repo, ticket and suggested branch.
func TestStatusForABuild(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "S-0036", "api")
	code, out := captureStdout(t, func() int { return runStatus(root) })
	for _, want := range []string{"change:   S-0036", "loop:     build", "repo:     api", "gates:    ticket=draft requirements=", "ticket:   S-0036 Fix login", "branch:   s-0036-fix-login"} {
		if !strings.Contains(out, want) {
			t.Errorf("status lacks %q:\n%s", want, out)
		}
	}
	if code != 0 {
		t.Fatalf("status = %d", code)
	}
}
