package cli

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/state"
)

// Security finding 2 (S-0040): a committed settings file can't point the intake
// store, the ADR folder or the sync file outside the workspace.
func TestSettingsPathsMustStayInside(t *testing.T) {
	for _, key := range []string{"sourceDir", "adrDir", "syncFile"} {
		t.Run(key, func(t *testing.T) {
			root := workspace(t)
			escape := filepath.Join(filepath.Dir(root), "escape-"+key)
			p := filepath.Join(root, ".daiku", "settings.json")
			if err := os.WriteFile(p, []byte(`{"`+key+`": "../escape-`+key+`"}`), 0o644); err != nil {
				t.Fatal(err)
			}
			for _, cmd := range []struct {
				name string
				run  func() int
			}{
				{"source new", func() int { return runSource(root, []string{"new", "--title", "Next"}) }},
				{"adr new", func() int { return runAdr(root, []string{"new", "--title", "Use JWT"}) }},
				{"change new", func() int { return runChange(root, []string{"new", "Gateway"}) }},
			} {
				code, stderr := captureStderr(t, cmd.run)
				if code == 0 || !strings.Contains(stderr, "must be a path inside the workspace") {
					t.Fatalf("%s = %d, stderr %q", cmd.name, code, stderr)
				}
			}
			if _, err := os.Stat(escape); err == nil {
				t.Fatalf("%s was created outside the workspace", escape)
			}
		})
	}
}

// Security finding 3 (S-0040): plan-command writes refuse a symlinked folder
// inside plan/ or ideas/ and leave the link's target empty.
func TestPlanWritesRefuseSymlinkedFolders(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	cases := []struct {
		name, folder string
		setup        func(t *testing.T) string
		run          func(root string) int
	}{
		{"source new", "plan/00_intake", workspace,
			func(root string) int { return runSource(root, []string{"new", "--title", "Next"}) }},
		{"adr new", "plan/04_architecture/adr", workspace,
			func(root string) int { return runAdr(root, []string{"new", "--title", "Use JWT"}) }},
		{"tickets new", "plan/specs/1-auth-flow/tickets", citeWorkspace,
			func(root string) int { return runTickets(root, []string{"new", "story", "Login", "--repo", "app"}) }},
		{"idea new", "ideas", workspace,
			func(root string) int { return runIdea(root, []string{"new", "Faster imports"}) }},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			root := c.setup(t)
			outside := t.TempDir()
			link := filepath.Join(root, filepath.FromSlash(c.folder))
			if err := os.RemoveAll(link); err != nil {
				t.Fatal(err)
			}
			if err := os.MkdirAll(filepath.Dir(link), 0o755); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(outside, link); err != nil {
				t.Fatal(err)
			}
			code, stderr := captureStderr(t, func() int { return c.run(root) })
			if code == 0 || !strings.Contains(stderr, "is a symlink") {
				t.Fatalf("%s = %d, stderr %q", c.name, code, stderr)
			}
			if n := entries(t, outside); n != 0 {
				t.Fatalf("wrote %d entries outside the workspace", n)
			}
		})
	}
}

// Security finding 3 (S-0040): a regenerated overview README isn't written
// through a symlink in its place, by daiku overview or by the refresh after a
// ticket command.
func TestOverviewRefusesSymlinkedReadme(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	root := citeWorkspace(t)
	if code := runOverview(root, nil); code != 0 {
		t.Fatalf("first overview = %d", code)
	}
	readme := filepath.Join(root, "plan", "specs", "1-auth-flow", "README.md")
	// The target holds the README, so only the write is what's refused.
	content, err := os.ReadFile(readme)
	if err != nil {
		t.Fatal(err)
	}
	victim := filepath.Join(t.TempDir(), "victim")
	if err := os.WriteFile(victim, content, 0o644); err != nil {
		t.Fatal(err)
	}
	os.Remove(readme)
	if err := os.Symlink(victim, readme); err != nil {
		t.Fatal(err)
	}
	// A new ticket changes the overview; its automatic refresh is refused too.
	newTicket(t, root, "story", "Login", "--repo", "app")
	code, stderr := captureStderr(t, func() int { return runOverview(root, nil) })
	if code == 0 || !strings.Contains(stderr, "is a symlink") {
		t.Fatalf("overview = %d, stderr %q", code, stderr)
	}
	if got, _ := os.ReadFile(victim); string(got) != string(content) {
		t.Fatalf("victim = %q", got)
	}
}

// Code review finding 9 (S-0040): a ticket command that's refused because LOG.md
// is a symlink leaves its ticket as it was, instead of changing it unlogged.
func TestTicketCommandsRefuseBeforeWriting(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	for _, c := range []struct {
		name  string
		setup func(t *testing.T) string
		run   func(root string) int
	}{
		{"ready", func(t *testing.T) string { return readyWorkspace(t, "breakdown") },
			func(root string) int { return runTickets(root, []string{"ready", "S-0001"}) }},
		{"mark-published", func(t *testing.T) string {
			root := workspace(t)
			ticketRepo(t, root)
			startPublish(t, root)
			newReady(t, root, "S-0001", "story", "Login", "--repo", "app")
			return root
		}, func(root string) int {
			return runTickets(root, []string{"mark-published", "S-0001", "--external", "github=o/r#1"})
		}},
		{"unfreeze", func(t *testing.T) string {
			root := workspace(t)
			ticketRepo(t, root)
			startPublish(t, root)
			newReady(t, root, "S-0001", "story", "Login", "--repo", "app")
			if code, out := markPublished(t, root, "S-0001", "--external", "github=o/r#1"); code != 0 {
				t.Fatalf("mark-published = %d: %s", code, out)
			}
			return root
		}, func(root string) int {
			return runTickets(root, []string{"unfreeze", "S-0001", "--reason", "rework"})
		}},
	} {
		t.Run(c.name, func(t *testing.T) {
			root := c.setup(t)
			before := readFile(t, ticketPath(root, "S-0001"))
			logPath := filepath.Join(root, "LOG.md")
			victim := filepath.Join(t.TempDir(), "victim")
			if err := os.WriteFile(victim, []byte(readFile(t, logPath)), 0o644); err != nil {
				t.Fatal(err)
			}
			logBefore := readFile(t, victim)
			os.Remove(logPath)
			if err := os.Symlink(victim, logPath); err != nil {
				t.Fatal(err)
			}
			code, stderr := captureStderr(t, func() int { return c.run(root) })
			if code == 0 || !strings.Contains(stderr, "LOG.md is a symlink") {
				t.Fatalf("%s = %d, stderr %q", c.name, code, stderr)
			}
			if after := readFile(t, ticketPath(root, "S-0001")); after != before {
				t.Fatalf("a refused %s changed the ticket:\n%s", c.name, after)
			}
			if readFile(t, victim) != logBefore {
				t.Fatal("LOG.md's target was written")
			}
		})
	}
}

// AC8: a symlinked plan README refuses the build's gate, reopen and close
// before any state changes, and the link's target is untouched.
func TestSymlinkedPlanOverviewRefusesBuildTransitions(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks")
	}
	root := plannedBuild(t, false)
	gateAll(t, root, []string{"ticket", "approved"})
	target := filepath.Join(t.TempDir(), "elsewhere.md")
	if err := os.WriteFile(target, []byte("outside\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	readme := filepath.Join(root, filepath.FromSlash(authReadme))
	if err := os.Remove(readme); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, readme); err != nil {
		t.Fatal(err)
	}
	stateBefore, logBefore := readFile(t, state.StatePath(root)), readLog(t, root)
	for _, tc := range []struct {
		name string
		run  func() int
	}{
		{"gate", func() int { return runGate(root, []string{"requirements", "approved"}) }},
		{"reopen", func() int { return runReopen(root, []string{"ticket", "--reason", "r"}) }},
		{"close", func() int { return runChange(root, []string{"close", "S-0001", "--reason", "r"}) }},
	} {
		code, stderr := captureStderr(t, tc.run)
		if code == 0 || !strings.Contains(stderr, "is a symlink") || !strings.Contains(stderr, "README.md") {
			t.Errorf("%s = %d: %s", tc.name, code, stderr)
		}
	}
	if readFile(t, state.StatePath(root)) != stateBefore || readLog(t, root) != logBefore {
		t.Fatal("a refused transition changed STATE.json or LOG.md")
	}
	if got := readFile(t, target); got != "outside\n" {
		t.Fatalf("link target = %q", got)
	}
	if st, err := state.LoadState(root); err != nil || st.Name != "S-0001" {
		t.Fatalf("active = %+v, %v", st, err)
	}
}

// AC8 for a plan's own README: its gate refuses too.
func TestSymlinkedOverviewRefusesAPlanGate(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks")
	}
	root := vcsWorkspace(t, `{"vcs": "none"}`)
	if code := runChange(root, []string{"new", "Auth flow"}); code != 0 {
		t.Fatal("change new")
	}
	target := filepath.Join(t.TempDir(), "elsewhere.md")
	if err := os.WriteFile(target, []byte("outside\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	readme := filepath.Join(root, filepath.FromSlash(authReadme))
	os.Remove(readme)
	if err := os.Symlink(target, readme); err != nil {
		t.Fatal(err)
	}
	stateBefore := readFile(t, state.StatePath(root))
	code, stderr := gateStderr(t, root, "intake", "approved")
	if code == 0 || !strings.Contains(stderr, "is a symlink") {
		t.Fatalf("gate = %d: %s", code, stderr)
	}
	if readFile(t, state.StatePath(root)) != stateBefore || readFile(t, target) != "outside\n" {
		t.Fatal("the refused gate changed something")
	}
}

// AC8 for build new: a symlinked plan README refuses it before the change is
// recorded.
func TestSymlinkedPlanOverviewRefusesBuildNew(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks")
	}
	root := readyWorkspace(t, "breakdown")
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"ready", "S-0001"}) }); code != 0 {
		t.Fatalf("ready = %d: %s", code, out)
	}
	target := filepath.Join(t.TempDir(), "elsewhere.md")
	if err := os.WriteFile(target, []byte("outside\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	readme := filepath.Join(root, filepath.FromSlash(authReadme))
	os.Remove(readme)
	if err := os.Symlink(target, readme); err != nil {
		t.Fatal(err)
	}
	stateBefore := readFile(t, state.StatePath(root))
	code, stderr := captureStderr(t, func() int { return runBuild(root, []string{"new", "S-0001"}) })
	if code == 0 || !strings.Contains(stderr, "is a symlink") {
		t.Fatalf("build new = %d: %s", code, stderr)
	}
	if readFile(t, state.StatePath(root)) != stateBefore || readFile(t, target) != "outside\n" || isDir(filepath.Join(root, "build")) {
		t.Fatal("the refused build new changed something")
	}
}

// Code review finding 1: closing a plan folder STATE.json doesn't record, whose
// README is a symlink, refuses before the close is recorded.
func TestSymlinkedOverviewRefusesClosingAnUnrecordedPlan(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks")
	}
	root := vcsWorkspace(t, `{"vcs": "none"}`)
	if code := runChange(root, []string{"new", "Auth flow"}); code != 0 {
		t.Fatal("change new")
	}
	ghost := filepath.Join(root, "plan", "specs", "9-ghost")
	if err := os.MkdirAll(ghost, 0o755); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(t.TempDir(), "elsewhere.md")
	if err := os.WriteFile(target, []byte("outside\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(ghost, "README.md")); err != nil {
		t.Fatal(err)
	}
	stateBefore, logBefore := readFile(t, state.StatePath(root)), readLog(t, root)
	code, stderr := captureStderr(t, func() int { return runChange(root, []string{"close", "9-ghost", "--reason", "r"}) })
	if code == 0 || !strings.Contains(stderr, "is a symlink") {
		t.Fatalf("close = %d: %s", code, stderr)
	}
	if readFile(t, state.StatePath(root)) != stateBefore || readLog(t, root) != logBefore || readFile(t, target) != "outside\n" {
		t.Fatal("the refused close changed something")
	}
}
