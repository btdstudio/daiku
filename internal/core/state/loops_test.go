package state

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/btdstudio/daiku/internal/core/settings"
	"github.com/btdstudio/daiku/internal/core/ticket"
)

var (
	planMachine = Machine{Tool: "daiku", Phases: []Phase{
		{Name: "intake", Kind: Gated},
		{Name: "publish", Kind: Run},
	}}
	buildMachine = Machine{Tool: "daiku", Phases: []Phase{
		{Name: "ticket", Kind: Gated},
		{Name: "ship", Kind: Run},
	}}
)

// daikuStore is a store with daiku's two loops, as S-0040 and S-0041 will build
// it, around two-phase sample machines.
func daikuStore() Store {
	return Store{
		Tool: "daiku",
		Loops: map[string]Loop{
			PlanLoopName:  PlanLoop(planMachine),
			BuildLoopName: BuildLoop(buildMachine, ticket.IDRe),
		},
		VCS:  settings.VCSNone,
		Now:  func() time.Time { return time.Date(2026, 11, 14, 0, 0, 0, 0, time.UTC) },
		Warn: &bytes.Buffer{},
	}
}

func isDir(t *testing.T, p string) {
	t.Helper()
	if info, err := os.Stat(p); err != nil || !info.IsDir() {
		t.Errorf("%s isn't a folder: %v", p, err)
	}
}

// AC2: a plan change and a build change share one STATE.json, each records its
// loop, and each folder is where its loop puts it.
func TestPlanAndBuildChangesInOneStateFile(t *testing.T) {
	root, s := t.TempDir(), daikuStore()
	if _, err := s.Seed(root, "1-gateway", PlanLoopName, SeedOptions{Repos: []string{"api", "web"}}); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Seed(root, "S-0036", BuildLoopName, SeedOptions{Repos: []string{"api"}}); err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(s.StatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	var sf struct {
		Changes map[string]struct {
			Loop  string `json:"loop"`
			Phase string `json:"phase"`
		} `json:"changes"`
	}
	if err := json.Unmarshal(raw, &sf); err != nil {
		t.Fatal(err)
	}
	if got := sf.Changes["1-gateway"]; got.Loop != "plan" || got.Phase != "intake" {
		t.Errorf("1-gateway = %+v", got)
	}
	if got := sf.Changes["S-0036"]; got.Loop != "build" || got.Phase != "ticket" {
		t.Errorf("S-0036 = %+v", got)
	}
	isDir(t, filepath.Join(root, "plan", "specs", "1-gateway"))
	isDir(t, filepath.Join(root, "build", "api", "changes", "S-0036"))

	// Each change follows its own machine and folder.
	if _, err := s.Advance(root, "ticket", Approved, ""); err != nil {
		t.Fatalf("build gate: %v", err)
	}
	if _, err := s.Switch(root, "1-gateway"); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Advance(root, "ticket", Approved, ""); err == nil {
		t.Fatal("a plan change accepted a build phase")
	}
	if _, err := s.Advance(root, "intake", Approved, ""); err != nil {
		t.Fatalf("plan gate: %v", err)
	}
	if id, err := s.NextID(root, PlanLoopName); err != nil || id != 2 {
		t.Fatalf("NextID = %d, %v", id, err)
	}
	known, err := s.Known(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(known) != 2 || known[0].Name != "S-0036" || known[0].Loop != "build" || known[1].Name != "1-gateway" || known[1].Loop != "plan" {
		t.Fatalf("known = %+v", known)
	}
}

// AC3: a build change is named by a ticket id, and nothing else.
func TestBuildChangeNames(t *testing.T) {
	for _, name := range []string{"7-gateway", "../x", "S-1", "x-1234", "X-#12", ""} {
		root, s := t.TempDir(), daikuStore()
		if _, err := s.Seed(root, name, BuildLoopName, SeedOptions{Repos: []string{"api"}}); err == nil || !strings.Contains(err.Error(), "invalid change name") {
			t.Errorf("Seed(%q) err = %v", name, err)
		}
		if _, err := os.Stat(filepath.Join(root, "build")); !os.IsNotExist(err) {
			t.Errorf("Seed(%q) created build/", name)
		}
	}
	root, s := t.TempDir(), daikuStore()
	for _, name := range []string{"S-0036", "N-0001", "X-1234"} {
		if _, err := s.Seed(root, name, BuildLoopName, SeedOptions{Repos: []string{"api"}}); err != nil {
			t.Errorf("Seed(%q) err = %v", name, err)
		}
		isDir(t, filepath.Join(root, "build", "api", "changes", name))
	}
}

// A plan change keeps its numbered grammar.
func TestPlanChangeNames(t *testing.T) {
	root, s := t.TempDir(), daikuStore()
	if _, err := s.Seed(root, "S-0036", PlanLoopName, SeedOptions{}); err == nil {
		t.Fatal("a plan change took a ticket id")
	}
	if _, err := s.Seed(root, "1-gateway", PlanLoopName, SeedOptions{}); err != nil {
		t.Fatal(err)
	}
}

func TestBuildChangeNeedsASafeRepo(t *testing.T) {
	for _, repos := range [][]string{nil, {}, {"../evil"}, {"a/b"}, {".."}, {"API"}} {
		root, s := t.TempDir(), daikuStore()
		if _, err := s.Seed(root, "S-0036", BuildLoopName, SeedOptions{Repos: repos}); err == nil {
			t.Errorf("Seed with repos %q succeeded", repos)
		}
		if _, err := os.Stat(s.StatePath(root)); !os.IsNotExist(err) {
			t.Errorf("repos %q: STATE.json written", repos)
		}
		entries, _ := os.ReadDir(root)
		if len(entries) != 0 {
			t.Errorf("repos %q: left %d entries behind", repos, len(entries))
		}
		if _, err := os.Stat(filepath.Join(filepath.Dir(root), "evil")); !os.IsNotExist(err) {
			t.Errorf("repos %q: a folder escaped the workspace", repos)
		}
	}
}

func TestBuildLoopHasNoNumberedIDs(t *testing.T) {
	root, s := t.TempDir(), daikuStore()
	if _, err := s.NextID(root, BuildLoopName); err == nil {
		t.Fatal("NextID worked for the build loop")
	}
}

func TestBuildLoopRequiresANameRule(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("BuildLoop(m, nil) didn't panic")
		}
	}()
	BuildLoop(buildMachine, nil)
}

// Under vcs git, a build change's folder, its ChangeDirs and every transition
// are committed, and the tree ends clean.
func TestBuildChangeCommitsUnderGit(t *testing.T) {
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not installed")
	}
	root := t.TempDir()
	git := func(args ...string) string {
		t.Helper()
		cmd := exec.Command("git", append([]string{"-C", root}, args...)...)
		cmd.Env = append(os.Environ(), "GIT_CONFIG_GLOBAL=/dev/null", "GIT_CONFIG_SYSTEM=/dev/null")
		out, err := cmd.CombinedOutput()
		if err != nil {
			t.Fatalf("git %v: %v: %s", args, err, out)
		}
		return string(out)
	}
	git("init", "-q")
	git("config", "user.email", "qa@example.com")
	git("config", "user.name", "qa")
	git("config", "commit.gpgsign", "false")
	git("commit", "-q", "--allow-empty", "-m", "root")

	s := daikuStore()
	s.VCS, s.Commit = settings.VCSGit, true
	build := s.Loops[BuildLoopName]
	build.ChangeDirs = []string{"build/notes"}
	s.Loops[BuildLoopName] = build

	if _, err := s.Seed(root, "S-0036", BuildLoopName, SeedOptions{Repos: []string{"api"}}); err != nil {
		t.Fatal(err)
	}
	note := filepath.Join(root, "build", "notes", "S-0036", "n.md")
	if err := os.MkdirAll(filepath.Dir(note), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(note, []byte("note\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	design := filepath.Join(root, "build", "api", "changes", "S-0036", "design.md")
	if err := os.WriteFile(design, []byte("design\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Advance(root, "ticket", Approved, ""); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Reopen(root, "ticket", "redo"); err != nil {
		t.Fatal(err)
	}
	if _, err := s.CloseChange(root, "S-0036", "done"); err != nil {
		t.Fatal(err)
	}

	log := git("log", "--name-only", "--format=%s")
	for _, want := range []string{
		"daiku: S-0036: created",
		"build/api/changes/S-0036/.gitkeep",
		"daiku: S-0036: ticket → approved",
		"build/api/changes/S-0036/design.md",
		"build/notes/S-0036/n.md",
		"daiku: S-0036: reopened ticket (redo)",
		"daiku: S-0036: closed (done)",
	} {
		if !strings.Contains(log, want) {
			t.Errorf("git log is missing %q:\n%s", want, log)
		}
	}
	if st := git("status", "--porcelain"); strings.TrimSpace(st) != "" {
		t.Errorf("uncommitted after the transitions:\n%s", st)
	}
}
