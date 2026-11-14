package cli

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/build/inbox"
)

// AC4 / spec R24: with no active plan change, tickets new --inbox writes the next
// N- task to build/tickets/, and doesn't create the build room.
func TestTicketsNewInbox(t *testing.T) {
	root := rootOnly(t)
	if code := repoCmd(root, "add", "api", "https://github.com/org/api"); code != 0 {
		t.Fatal("repo add")
	}
	code, out := captureStdout(t, func() int {
		return runTickets(root, []string{"new", "task", "Fix login", "--repo", "api", "--inbox"})
	})
	if code != 0 || !strings.Contains(out, "created N-0001") {
		t.Fatalf("tickets new --inbox = %d:\n%s", code, out)
	}
	got, err := inbox.Read(root, "N-0001")
	if err != nil || got.Title != "Fix login" || got.Repo != "api" || got.Status != "draft" {
		t.Fatalf("ticket = %+v, %v", got, err)
	}
	if isDir(filepath.Join(root, "build", "01_design")) || isDir(filepath.Join(root, "plan")) {
		t.Fatal("tickets new --inbox created a room")
	}
	// No active change at all also goes to the inbox, with or without --inbox.
	if code := runTickets(root, []string{"new", "task", "Second", "--repo", "api"}); code != 0 {
		t.Fatal("tickets new without --inbox and no active change")
	}
	if _, err := inbox.Read(root, "N-0002"); err != nil {
		t.Fatal(err)
	}
}

// With an open plan change active, tickets new still plans a ticket unless
// --inbox says otherwise; with a build active, it goes to the inbox.
func TestTicketsNewPicksThePlanOrTheInbox(t *testing.T) {
	root := workspace(t)
	ticketRepo(t, root)
	if code := runTickets(root, []string{"new", "story", "Login", "--repo", "app"}); code != 0 {
		t.Fatal("planned ticket")
	}
	if _, err := os.Stat(ticketPath(root, "S-0001")); err != nil {
		t.Fatalf("no planned ticket: %v", err)
	}
	if code := runTickets(root, []string{"new", "task", "Hotfix", "--repo", "app", "--inbox"}); code != 0 {
		t.Fatal("--inbox with a plan active")
	}
	if _, err := inbox.Read(root, "N-0001"); err != nil {
		t.Fatal(err)
	}
	writeBuildChange(t, root)
	if code := runSwitch(root, []string{"S-0036"}); code != 0 {
		t.Fatal("switch to the build")
	}
	if code := runTickets(root, []string{"new", "task", "Follow-up", "--repo", "app"}); code != 0 {
		t.Fatal("tickets new with a build active")
	}
	if _, err := inbox.Read(root, "N-0002"); err != nil {
		t.Fatal(err)
	}
}

func TestTicketsNewInboxRefusals(t *testing.T) {
	root := rootOnly(t)
	if code := repoCmd(root, "add", "api", "https://github.com/org/api"); code != 0 {
		t.Fatal("repo add")
	}
	for _, tc := range []struct {
		args []string
		want string
	}{
		{[]string{"story", "X", "--repo", "api", "--inbox"}, "an inbox ticket is a task"},
		{[]string{"task", "X", "--inbox"}, "needs a repo"},
		{[]string{"task", "X", "--repo", "web", "--inbox"}, `repo "web" isn't registered`},
		{[]string{"task", "X", "--repo", "api", "--parent", "F-0001", "--inbox"}, "no --parent, --depends or --requirements"},
	} {
		code, stderr := captureStderr(t, func() int { return runTickets(root, append([]string{"new"}, tc.args...)) })
		if code != 1 || !strings.Contains(stderr, tc.want) {
			t.Errorf("tickets new %v = %d, stderr %q", tc.args, code, stderr)
		}
	}
	if entries, _ := os.ReadDir(inbox.Dir(root)); len(entries) != 0 {
		t.Fatalf("refusals wrote %d tickets", len(entries))
	}
}

func writeDraft(t *testing.T, external, title string) string {
	t.Helper()
	p := filepath.Join(t.TempDir(), "draft.md")
	text := "---\nschema: 1\nid: DRAFT\ntype: task\ntitle: " + title + "\nrepo: api\nlabels: []\nstatus: draft\nexternal:\n  " + external + "\n---\n\nFrom the PM tool.\n"
	if err := os.WriteFile(p, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	return p
}

// AC5 / spec R25, R26: a pulled item is X-<pm-id>; pulling it again replaces it.
func TestTicketsPull(t *testing.T) {
	root := rootOnly(t)
	code, out := captureStdout(t, func() int { return runTickets(root, []string{"pull", writeDraft(t, `github: "#1234"`, "First")}) })
	if code != 0 || !strings.Contains(out, "pulled X-1234 build/tickets/X-1234.md") {
		t.Fatalf("pull = %d:\n%s", code, out)
	}
	if code := runTickets(root, []string{"pull", writeDraft(t, "github: org/api#1234", "Second")}); code != 0 {
		t.Fatal("second pull")
	}
	all, problems, err := inbox.List(root)
	if err != nil || len(problems) != 0 || len(all) != 1 || all[0].ID != "X-1234" || all[0].Title != "Second" {
		t.Fatalf("inbox = %+v, %v, %v", all, problems, err)
	}
	if log := readLog(t, root); strings.Count(log, "X-1234: pulled from github") != 2 {
		t.Fatalf("log = %s", log)
	}
	if isDir(filepath.Join(root, "build", "01_design")) {
		t.Fatal("tickets pull created the build room")
	}
}

// AC12 (pull): with LOG.md a symlink, pull refuses before writing the ticket.
func TestTicketsPullRefusesASymlinkedLog(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	root := rootOnly(t)
	logPath := filepath.Join(root, "LOG.md")
	victim := filepath.Join(t.TempDir(), "victim")
	if err := os.WriteFile(victim, []byte("keep\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	os.Remove(logPath)
	if err := os.Symlink(victim, logPath); err != nil {
		t.Fatal(err)
	}
	code, stderr := captureStderr(t, func() int { return runTickets(root, []string{"pull", writeDraft(t, `github: "#1"`, "x")}) })
	if code != 1 || !strings.Contains(stderr, "LOG.md is a symlink") {
		t.Fatalf("pull = %d: %s", code, stderr)
	}
	if _, err := os.Stat(inbox.Path(root, "X-1")); !os.IsNotExist(err) {
		t.Fatal("a refused pull wrote the ticket")
	}
	if readFile(t, victim) != "keep\n" {
		t.Fatal("LOG.md's target was written")
	}
}

// AC6 / spec R27: an X- ticket under plan/specs/ is an error; the inbox's own
// tickets are checked too.
func TestTicketsCheckCoversTheInbox(t *testing.T) {
	root := workspace(t)
	if code := repoCmd(root, "add", "api", "https://github.com/org/api"); code != 0 {
		t.Fatal("repo add")
	}
	if code := runTickets(root, []string{"pull", writeDraft(t, `github: "#5"`, "Pulled")}); code != 0 {
		t.Fatal("pull")
	}
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"check"}) }); code != 0 || !strings.Contains(out, "1 tickets ok") {
		t.Fatalf("check with a valid inbox = %d:\n%s", code, out)
	}
	misplaced := filepath.Join(activeChangeDir(t, root), "tickets", "X-9.md")
	if err := os.MkdirAll(filepath.Dir(misplaced), 0o755); err != nil {
		t.Fatal(err)
	}
	data := strings.ReplaceAll(readFile(t, inbox.Path(root, "X-5")), "X-5", "X-9")
	data = strings.Replace(data, `"#5"`, `"#9"`, 1)
	if err := os.WriteFile(misplaced, []byte(data), 0o644); err != nil {
		t.Fatal(err)
	}
	code, out := captureStdout(t, func() int { return runTickets(root, []string{"check"}) })
	if code != 1 || !strings.Contains(out, "✗ X-9: pulled tickets live in build/tickets/") {
		t.Fatalf("check with a misplaced X- = %d:\n%s", code, out)
	}
	if err := os.Remove(misplaced); err != nil {
		t.Fatal(err)
	}
	broken := strings.Replace(readFile(t, inbox.Path(root, "X-5")), "repo: api", "repo: nowhere", 1)
	if err := os.WriteFile(inbox.Path(root, "X-5"), []byte(broken), 0o644); err != nil {
		t.Fatal(err)
	}
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"check"}) }); code != 1 || !strings.Contains(out, "X-5") {
		t.Fatalf("check with an unregistered inbox repo = %d:\n%s", code, out)
	}
}
