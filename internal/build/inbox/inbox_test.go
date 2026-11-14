package inbox

import (
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/ticket"
)

// Ported from tomo issues (af54ad2): the store moved to build/tickets/.
func TestNewCreatesValidNativeTicket(t *testing.T) {
	root := t.TempDir()
	created, err := New(root, NewOptions{Title: "Build: the thing", Repo: "app", Labels: []string{"docs", "ui"}, Body: "Wire up the store."})
	if err != nil {
		t.Fatal(err)
	}
	if created.ID != "N-0001" || created.Status != ticket.StatusDraft || created.Type != ticket.Task {
		t.Fatalf("created = %+v", created)
	}
	if !strings.HasSuffix(created.Path, filepath.Join("build", "tickets", "N-0001.md")) {
		t.Fatalf("path = %q", created.Path)
	}
	read, err := Read(root, "N-0001")
	if err != nil {
		t.Fatal(err)
	}
	if read.Title != "Build: the thing" || !reflect.DeepEqual(read.Labels, []string{"docs", "ui"}) || !strings.Contains(read.Body, "Wire up the store.") {
		t.Fatalf("read = %+v", read)
	}
	if problems := ticket.ValidateFile(read); len(problems) != 0 {
		t.Fatalf("problems = %v", problems)
	}
}

// Ported from tomo TestIDsSkipImportedTickets: the next N- follows the highest
// sequence in the inbox, whatever its prefix.
func TestIDsFollowTheHighestNumber(t *testing.T) {
	root := t.TempDir()
	for range 3 {
		if _, err := New(root, NewOptions{Title: "x", Repo: "app"}); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Remove(Path(root, "N-0002")); err != nil {
		t.Fatal(err)
	}
	created, err := New(root, NewOptions{Title: "y", Repo: "app"})
	if err != nil {
		t.Fatal(err)
	}
	if created.ID != "N-0004" {
		t.Fatalf("id = %s", created.ID)
	}
}

func TestReadAcceptsANumberAndReportsMissing(t *testing.T) {
	root := t.TempDir()
	if _, err := New(root, NewOptions{Title: "x", Repo: "app"}); err != nil {
		t.Fatal(err)
	}
	if got, err := Read(root, "1"); err != nil || got.ID != "N-0001" {
		t.Fatalf("got = %+v, %v", got, err)
	}
	if _, err := Read(root, "N-0009"); err == nil || !strings.Contains(err.Error(), "not found") {
		t.Fatalf("err = %v", err)
	}
}

// Ported from tomo TestCustomDirAndEmptyStore, minus the custom dir daiku doesn't have.
func TestEmptyInbox(t *testing.T) {
	all, problems, err := List(t.TempDir())
	if err != nil || len(all) != 0 || len(problems) != 0 {
		t.Fatalf("empty = %v, %v, %v", all, problems, err)
	}
}

// tomo's List failed on an unreadable file; daiku's reports it as a problem, so
// tickets check can list it, and New still refuses to allocate.
func TestListReportsUnreadableFile(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(Dir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(Dir(root), "N-0001.md"), []byte("not a ticket"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, problems, err := List(root); err != nil || len(problems) != 1 {
		t.Fatalf("problems = %v, %v", problems, err)
	}
	if _, err := New(root, NewOptions{Title: "x", Repo: "app"}); err == nil || !strings.Contains(err.Error(), "unreadable") {
		t.Fatalf("New err = %v", err)
	}
}

func TestNewNeedsARepo(t *testing.T) {
	if _, err := New(t.TempDir(), NewOptions{Title: "x"}); err == nil || !strings.Contains(err.Error(), "--repo") {
		t.Fatalf("err = %v", err)
	}
}

// D10: a planned ticket in the inbox is reported.
func TestListReportsAPlannedTicket(t *testing.T) {
	root := t.TempDir()
	planned, err := ticket.New(ticket.Ticket{ID: "S-0014", Type: ticket.Story, Title: "x", Repo: "app"})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(Dir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(Path(root, "S-0014"), planned.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}
	_, problems, err := List(root)
	if err != nil || len(problems) != 1 || !strings.Contains(problems[0].String(), "plan/specs/") {
		t.Fatalf("problems = %v, %v", problems, err)
	}
}

func TestPMID(t *testing.T) {
	for value, want := range map[string]string{
		"org/app#1234": "1234",
		"#1234":        "1234",
		"1234":         "1234",
		"PROJ-123":     "PROJ-123",
		"a#b#77":       "77",
	} {
		if got := PMID(value); got != want {
			t.Errorf("PMID(%q) = %q, want %q", value, got, want)
		}
	}
}

func draft(external, title string) []byte {
	return []byte("---\nschema: 1\nid: DRAFT\ntype: task\ntitle: " + title + "\nrepo: api\nchange: 7-gateway\nlabels: [bug]\nstatus: draft\n" + external + "origin:\n  workspace: w\n  path: p\n---\n\nThe body.\n")
}

// AC5 / spec R25, R26: a pulled ticket is X-<pm-id>; pulling again replaces it.
func TestPullWritesAndReplaces(t *testing.T) {
	root := t.TempDir()
	got, err := Pull(root, draft("external:\n  github: \"#1234\"\n", "First"))
	if err != nil {
		t.Fatal(err)
	}
	if got.ID != "X-1234" || got.Path != Path(root, "X-1234") || got.Origin != (ticket.Origin{}) || got.Change != "" {
		t.Fatalf("got = %+v", got)
	}
	if !reflect.DeepEqual(got.Labels, []string{"bug"}) || got.External["github"] != "#1234" {
		t.Fatalf("fields lost: %+v", got)
	}
	if _, err := Pull(root, draft("external:\n  github: org/app#1234\n", "Second")); err != nil {
		t.Fatal(err)
	}
	all, problems, err := List(root)
	if err != nil || len(problems) != 0 || len(all) != 1 || all[0].Title != "Second" {
		t.Fatalf("inbox = %+v, %v, %v", all, problems, err)
	}
	if p := ticket.ValidateFile(all[0]); len(p) != 0 {
		t.Fatalf("problems = %v", p)
	}
}

func TestPullNeedsOneExternalID(t *testing.T) {
	root := t.TempDir()
	for name, ext := range map[string]string{
		"none": "",
		"two":  "external:\n  github: \"#1\"\n  ado: \"2\"\n",
	} {
		if _, err := Pull(root, draft(ext, "x")); err == nil || !strings.Contains(err.Error(), "exactly one external") {
			t.Errorf("%s: err = %v", name, err)
		}
	}
	if _, err := Pull(root, draft("external:\n  github: \"#\"\n", "x")); err == nil {
		t.Error("an external id with no letters or digits was accepted")
	}
	if entries, _ := os.ReadDir(Dir(root)); len(entries) != 0 {
		t.Fatalf("a refused pull wrote %d files", len(entries))
	}
}

// N3: the inbox isn't written through a symlink.
func TestInboxRefusesSymlinks(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	root, outside := t.TempDir(), t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "build"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, Dir(root)); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root, NewOptions{Title: "x", Repo: "app"}); err == nil || !strings.Contains(err.Error(), "is a symlink") {
		t.Fatalf("New err = %v", err)
	}
	if _, err := Pull(root, draft("external:\n  github: \"#1\"\n", "x")); err == nil || !strings.Contains(err.Error(), "is a symlink") {
		t.Fatalf("Pull err = %v", err)
	}
	if entries, _ := os.ReadDir(outside); len(entries) != 0 {
		t.Fatalf("wrote %d entries outside the workspace", len(entries))
	}
}
