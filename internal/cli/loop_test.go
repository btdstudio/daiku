package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/icm"
)

// rootOnly is a workspace as `daiku init` leaves it: the root tree, no rooms.
func rootOnly(t *testing.T) string {
	t.Helper()
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	root := t.TempDir()
	if _, err := icm.Scaffold(root, icm.Options{NoGit: true}, "none"); err != nil {
		t.Fatal(err)
	}
	return root
}

func isDir(p string) bool {
	info, err := os.Stat(p)
	return err == nil && info.IsDir()
}

// AC3: the first change new in a root-only workspace creates plan/ and its
// AGENTS.md sections and says so; the next one doesn't; build/ never appears.
func TestChangeNewCreatesThePlanRoom(t *testing.T) {
	root := rootOnly(t)
	code, out := captureStdout(t, func() int { return runChange(root, []string{"new", "Gateway"}) })
	if code != 0 || !strings.HasPrefix(out, "created room plan/\n") || !strings.Contains(out, "created change 1-gateway") {
		t.Fatalf("change new = %d:\n%s", code, out)
	}
	if !isDir(filepath.Join(root, "plan", "specs", "1-gateway")) || !isDir(filepath.Join(root, "plan", "00_intake")) {
		t.Error("plan room or change folder missing")
	}
	if isDir(filepath.Join(root, "build")) {
		t.Error("change new created build/")
	}
	agents, _ := os.ReadFile(filepath.Join(root, "AGENTS.md"))
	if !strings.Contains(string(agents), "## Plan loop") || !strings.Contains(string(agents), "## How to start") {
		t.Errorf("AGENTS.md lacks the plan sections:\n%s", agents)
	}
	code, out = captureStdout(t, func() int { return runChange(root, []string{"new", "Second"}) })
	if code != 0 || strings.Contains(out, "created room") {
		t.Fatalf("second change new = %d:\n%s", code, out)
	}
}

// idea promote is the other way into the plan room.
func TestIdeaPromoteCreatesThePlanRoom(t *testing.T) {
	root := rootOnly(t)
	if code := runIdea(root, []string{"new", "Faster imports"}); code != 0 {
		t.Fatal("idea new")
	}
	code, out := captureStdout(t, func() int { return runIdea(root, []string{"promote", "1", "--title", "Faster imports"}) })
	if code != 0 || !strings.HasPrefix(out, "created room plan/\n") {
		t.Fatalf("promote = %d:\n%s", code, out)
	}
}

// AC5: a plan change goes from intake to publish with gate, reopen, source, adr,
// tickets and overview, every file under plan/.
func TestPlanLoopEndToEnd(t *testing.T) {
	root := workspace(t)
	if code := runSource(root, []string{"new", "--title", "Extra context"}); code != 0 {
		t.Fatal("source new")
	}
	if _, err := os.Stat(filepath.Join(root, "plan", "00_intake", "2.md")); err != nil {
		t.Fatalf("source new didn't write plan/00_intake/2.md: %v", err)
	}
	if code := runAdr(root, []string{"new", "--title", "Use signed tokens"}); code != 0 {
		t.Fatal("adr new")
	}
	if _, err := os.Stat(filepath.Join(root, "plan", "04_architecture", "adr", "0001-use-signed-tokens.md")); err != nil {
		t.Fatalf("adr new didn't write under plan/04_architecture/adr: %v", err)
	}
	writeArtifacts(t, root)
	ticketRepo(t, root)
	if code := runTickets(root, []string{"new", "story", "Login", "--repo", "app"}); code != 0 {
		t.Fatal("tickets new")
	}
	fillCriteria(t, root, "S-0001")
	for _, args := range [][]string{
		{"intake", "approved"},
		{"research", "skipped", "--reason", "none"},
		{"spec", "approved"},
		{"architecture", "approved"},
	} {
		if code, stderr := gateStderr(t, root, args...); code != 0 {
			t.Fatalf("gate %v = %d:\n%s", args, code, stderr)
		}
	}
	if code := runReopen(root, []string{"spec", "--reason", "one more requirement"}); code != 0 {
		t.Fatal("reopen spec")
	}
	if g := gates(t, root); g["spec"] != "draft" || g["architecture"] != "" {
		t.Fatalf("after reopen: %v", g)
	}
	for _, args := range [][]string{
		{"spec", "approved"},
		{"architecture", "approved"},
		{"breakdown", "approved"},
		{"publish", "in-progress"},
	} {
		if code, stderr := gateStderr(t, root, args...); code != 0 {
			t.Fatalf("gate %v = %d:\n%s", args, code, stderr)
		}
	}
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"ready", "S-0001"}) }); code != 0 {
		t.Fatalf("ready = %d: %s", code, out)
	}
	if code, out := markPublished(t, root, "S-0001", "--external", "github=o/r#1"); code != 0 {
		t.Fatalf("mark-published = %d: %s", code, out)
	}
	if code, stderr := gateStderr(t, root, "publish", "complete"); code != 0 {
		t.Fatalf("publish complete = %d:\n%s", code, stderr)
	}
	readme := filepath.Join(activeChangeDir(t, root), "README.md")
	if !strings.HasPrefix(readme, filepath.Join(root, "plan", "specs")) {
		t.Fatalf("overview path %s isn't under plan/specs", readme)
	}
	if code := runOverview(root, []string{"--check"}); code != 0 {
		t.Fatalf("overview --check = %d after the gates regenerated it", code)
	}
	ticket, _ := os.ReadFile(filepath.Join(activeChangeDir(t, root), "tickets", "S-0001.md"))
	if !strings.Contains(string(ticket), "spec: plan/specs/") {
		t.Errorf("ticket's spec isn't under plan/specs:\n%s", ticket)
	}
}

// AC8: in a zumen workspace, status names zumen and writes nothing.
func TestStatusInAZumenWorkspace(t *testing.T) {
	for _, tool := range []string{"zumen", "tomo"} {
		root := t.TempDir()
		if err := os.Mkdir(filepath.Join(root, "."+tool), 0o755); err != nil {
			t.Fatal(err)
		}
		code, stderr := captureStderr(t, func() int { return runStatus(root) })
		if code != 1 || !strings.Contains(stderr, "a "+tool+" workspace") {
			t.Errorf("%s: status = %d, stderr %q", tool, code, stderr)
		}
		if entries, _ := os.ReadDir(root); len(entries) != 1 {
			t.Errorf("%s: status wrote %d entries", tool, len(entries)-1)
		}
	}
}

// writeBuildChange records a build change next to the workspace's plan change
// and makes it active, as a later version's `build new` will.
func writeBuildChange(t *testing.T, root string) {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join(root, "STATE.json"))
	if err != nil {
		t.Fatal(err)
	}
	build := `"S-0036": {"loop": "build", "phase": "ticket", "status": "draft", "gates": {"ticket": "draft"}, "sources": [], "repos": ["api"]},`
	text := strings.Replace(string(raw), `"changes": {`, `"changes": {`+"\n    "+build, 1)
	if err := os.WriteFile(filepath.Join(root, "STATE.json"), []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "build", "api", "changes", "S-0036"), 0o755); err != nil {
		t.Fatal(err)
	}
}

// AC11: with a build change recorded, status works with either active, and the
// verbs that act on a change refuse a build change clearly.
func TestBuildChangeActive(t *testing.T) {
	root := workspace(t)
	writeBuildChange(t, root)
	code, out := captureStdout(t, func() int { return runStatus(root) })
	if code != 0 || !strings.Contains(out, "change:   1-auth-flow") || strings.Contains(out, "loop:") {
		t.Fatalf("status with the plan change active = %d:\n%s", code, out)
	}
	if code, out := captureStdout(t, func() int { return runSwitch(root, nil) }); code != 0 || !strings.Contains(out, "S-0036  build  ticket draft") || !strings.Contains(out, "1-auth-flow  plan  intake draft") {
		t.Fatalf("switch list = %d:\n%s", code, out)
	}
	if code := runSwitch(root, []string{"S-0036"}); code != 0 {
		t.Fatal("switch to the build change")
	}
	code, out = captureStdout(t, func() int { return runStatus(root) })
	if code != 0 || !strings.Contains(out, "loop:     build") || !strings.Contains(out, "gates:    ticket=draft requirements=") {
		t.Fatalf("status with the build change active = %d:\n%s", code, out)
	}
	want := "daiku: S-0036 is a build change — build commands aren't in this version yet"
	for name, run := range map[string]func() int{
		"gate":         func() int { return runGate(root, []string{"ticket", "approved"}) },
		"reopen":       func() int { return runReopen(root, []string{"ticket", "--reason", "r"}) },
		"env":          func() int { return runEnv(root) },
		"change close": func() int { return runChangeClose(root, []string{"S-0036", "--reason", "r"}) },
		// Plan commands that use the active change (code review finding 1).
		"tickets new":      func() int { return runTickets(root, []string{"new", "story", "X", "--repo", "app"}) },
		"tickets ready":    func() int { return runTickets(root, []string{"ready", "--all"}) },
		"overview":         func() int { return runOverview(root, nil) },
		"overview <build>": func() int { return runOverview(root, []string{"S-0036"}) },
	} {
		if code, stderr := captureStderr(t, run); code != 1 || !strings.Contains(stderr, want) {
			t.Errorf("%s = %d, stderr %q", name, code, stderr)
		}
	}
	if g := readLog(t, root); strings.Contains(g, "S-0036") {
		t.Errorf("a refused build command logged:\n%s", g)
	}
	// Commands over the whole workspace still work, and see plan changes only.
	for name, args := range map[string][]string{"tickets check": {"check"}, "tickets status": {"status"}, "tickets order": {"order"}} {
		if code, stderr := captureStderr(t, func() int { return runTickets(root, args) }); code != 0 {
			t.Errorf("%s = %d, stderr %q", name, code, stderr)
		}
	}
	if _, err := os.Stat(filepath.Join(root, "plan", "specs", "S-0036")); !os.IsNotExist(err) {
		t.Error("a plan command made a plan/specs/S-0036 folder for a build change")
	}
}

// A plan-only workspace lists changes as zumen did, with no loop column.
func TestSwitchListHasNoLoopColumnWithoutBuilds(t *testing.T) {
	root := workspace(t)
	code, out := captureStdout(t, func() int { return runSwitch(root, nil) })
	if code != 0 || out != "1-auth-flow  intake draft\n" {
		t.Fatalf("switch = %d: %q", code, out)
	}
}
