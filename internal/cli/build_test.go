package cli

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/build/inbox"
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

// inboxRoot is a root-only workspace (as init leaves it, vcs: none) with repo api
// registered and one native ticket for it in the inbox.
func inboxRoot(t *testing.T) string {
	t.Helper()
	root := rootOnly(t)
	if code := repoCmd(root, "add", "api", "https://github.com/org/api", "--path", filepath.Join(t.TempDir(), "api")); code != 0 {
		t.Fatal("repo add api")
	}
	if _, err := inbox.New(root, inbox.NewOptions{Title: "Fix login", Repo: "api"}); err != nil {
		t.Fatal(err)
	}
	return root
}

// AC1 / spec R5, R16: the first build new creates the build room and the change,
// and never plan/.
func TestBuildNewFromTheInbox(t *testing.T) {
	root := inboxRoot(t)
	code, out := captureStdout(t, func() int { return runBuild(root, []string{"new", "1"}) })
	if code != 0 || !strings.HasPrefix(out, "created room build/\n") || !strings.Contains(out, "created build N-0001 for api (build/tickets/N-0001.md)") {
		t.Fatalf("build new = %d:\n%s", code, out)
	}
	if !isDir(filepath.Join(root, "build", "api", "changes", "N-0001")) || !isDir(filepath.Join(root, "build", "01_design")) {
		t.Error("build room or change folder missing")
	}
	if isDir(filepath.Join(root, "plan")) {
		t.Error("build new created plan/")
	}
	agents, _ := os.ReadFile(filepath.Join(root, "AGENTS.md"))
	if !strings.Contains(string(agents), "## Build loop") || strings.Contains(string(agents), "## Plan loop") {
		t.Errorf("AGENTS.md doesn't have exactly the build sections:\n%s", agents)
	}
	st, err := state.LoadState(root)
	if err != nil || st.Name != "N-0001" || st.Loop != state.BuildLoop || st.Phase != "ticket" {
		t.Fatalf("active = %+v, %v", st, err)
	}
	// The next build doesn't recreate the room.
	if _, err := inbox.New(root, inbox.NewOptions{Title: "Second", Repo: "api"}); err != nil {
		t.Fatal(err)
	}
	code, out = captureStdout(t, func() int { return runBuild(root, []string{"new", "N-0002"}) })
	if code != 0 || strings.Contains(out, "created room") {
		t.Fatalf("second build new = %d:\n%s", code, out)
	}
}

// AC2 / spec R17: a planned ticket is read in place; nothing is copied.
func TestBuildNewFromAPlannedTicket(t *testing.T) {
	root := readyWorkspace(t, "breakdown")
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"ready", "S-0001"}) }); code != 0 {
		t.Fatalf("ready = %d: %s", code, out)
	}
	if code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", "S-0001"}) }); code != 0 {
		t.Fatalf("build new = %d: %s", code, stderr)
	}
	st, err := state.LoadState(root)
	if err != nil {
		t.Fatal(err)
	}
	if st.Name != "S-0001" || len(st.Sources) != 1 || st.Sources[0] != "plan/specs/1-auth-flow/tickets/S-0001.md" || st.Repos[0] != "app" {
		t.Fatalf("build = %+v", st)
	}
	if entries, _ := os.ReadDir(filepath.Join(root, "build", "tickets")); len(entries) > 1 {
		t.Fatalf("build/tickets/ has %d entries; the ticket was copied", len(entries))
	}
	if _, err := os.Stat(filepath.Join(root, "build", "tickets", "S-0001.md")); !os.IsNotExist(err) {
		t.Fatal("the planned ticket was copied into the inbox")
	}
}

// D3: what build new refuses, writing nothing.
func TestBuildNewRefusals(t *testing.T) {
	writeInbox := func(t *testing.T, root, id, text string) {
		t.Helper()
		if err := os.MkdirAll(inbox.Dir(root), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(inbox.Path(root, id), []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	native := func(id, repo string) string {
		return "---\nschema: 1\nid: " + id + "\ntype: task\ntitle: x\nrepo: " + repo + "\nlabels: []\nstatus: draft\n---\n\n"
	}
	for _, tc := range []struct {
		name, id, want string
		setup          func(t *testing.T, root string)
	}{
		{"not an id", "../x", "isn't a ticket id", nil},
		{"unknown", "N-0009", "no ticket N-0009", nil},
		{"unregistered repo", "N-0005", `repo "web" isn't registered`, func(t *testing.T, root string) {
			writeInbox(t, root, "N-0005", native("N-0005", "web"))
		}},
		{"no repo", "N-0006", "repo is required for a task", func(t *testing.T, root string) {
			writeInbox(t, root, "N-0006", strings.Replace(native("N-0006", "api"), "repo: api\n", "", 1))
		}},
		{"planned id in the inbox", "S-0007", "planned ticket in the inbox", func(t *testing.T, root string) {
			writeInbox(t, root, "S-0007", strings.Replace(native("S-0007", "api"), "type: task", "type: story", 1))
		}},
		{"invalid ticket", "N-0008", "invalid ticket", func(t *testing.T, root string) {
			writeInbox(t, root, "N-0008", strings.Replace(native("N-0008", "api"), "type: task", "type: story", 1))
		}},
		{"already a change", "N-0001", "already a change", func(t *testing.T, root string) {
			if code := runBuild(root, []string{"new", "N-0001"}); code != 0 {
				t.Fatal("first build new")
			}
		}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := inboxRoot(t)
			if tc.setup != nil {
				tc.setup(t, root)
			}
			before := readFile(t, filepath.Join(root, "STATE.json"))
			code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", tc.id}) })
			if code != 1 || !strings.Contains(stderr, tc.want) {
				t.Fatalf("build new %s = %d, stderr %q", tc.id, code, stderr)
			}
			if readFile(t, filepath.Join(root, "STATE.json")) != before {
				t.Fatal("a refused build new changed STATE.json")
			}
		})
	}
}

// A planned ticket that isn't ready can't be built, and an id in two places is
// refused, naming both.
func TestBuildNewRefusesADraftPlannedTicketAndDuplicates(t *testing.T) {
	root := readyWorkspace(t, "breakdown")
	if code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", "S-0001"}) }); code != 1 || !strings.Contains(stderr, "S-0001 is draft; only a ready or published planned ticket") {
		t.Fatalf("draft = %d: %s", code, stderr)
	}
	pulled := "---\nschema: 1\nid: X-12\ntype: task\ntitle: x\nrepo: app\nlabels: []\nstatus: draft\nexternal:\n  github: \"#12\"\n---\n\n"
	for _, dir := range []string{filepath.Join(state.ChangeDir(root, "1-auth-flow"), "tickets"), inbox.Dir(root)} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(dir, "X-12.md"), []byte(pulled), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", "X-12"}) })
	if code != 1 || !strings.Contains(stderr, "more than one place: plan/specs/1-auth-flow/tickets/X-12.md, build/tickets/X-12.md") {
		t.Fatalf("duplicate = %d: %s", code, stderr)
	}
}

// AC12 (build new): with LOG.md a symlink, build new refuses before writing the
// room or the change, and the link's target is unchanged.
func TestBuildNewRefusesASymlinkedLog(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	root := inboxRoot(t)
	logPath := filepath.Join(root, "LOG.md")
	victim := filepath.Join(t.TempDir(), "victim")
	if err := os.WriteFile(victim, []byte("keep\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	os.Remove(logPath)
	if err := os.Symlink(victim, logPath); err != nil {
		t.Fatal(err)
	}
	before := readFile(t, filepath.Join(root, "STATE.json"))
	code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", "N-0001"}) })
	if code != 1 || !strings.Contains(stderr, "LOG.md is a symlink") {
		t.Fatalf("build new = %d: %s", code, stderr)
	}
	if readFile(t, filepath.Join(root, "STATE.json")) != before || isDir(filepath.Join(root, "build", "01_design")) || isDir(filepath.Join(root, "build", "api")) {
		t.Fatal("a refused build new wrote the room, the change or STATE.json")
	}
	if readFile(t, victim) != "keep\n" {
		t.Fatal("LOG.md's target was written")
	}
}
