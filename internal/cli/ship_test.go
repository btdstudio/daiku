package cli

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/build/ship"
	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/state"
)

// realGit skips unless git is on PATH, and isolates it from the developer's
// global and system config (ported from tomo).
func realGit(t *testing.T) {
	t.Helper()
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not on PATH")
	}
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("GIT_CONFIG_GLOBAL", os.DevNull)
	t.Setenv("GIT_CONFIG_NOSYSTEM", "1")
	t.Setenv("GIT_AUTHOR_NAME", "t")
	t.Setenv("GIT_AUTHOR_EMAIL", "t@example.com")
	t.Setenv("GIT_COMMITTER_NAME", "t")
	t.Setenv("GIT_COMMITTER_EMAIL", "t@example.com")
}

func gitCmd(t *testing.T, dir string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, out)
	}
	return string(out)
}

// shipRoot is a local workspace with repo api registered at a real git checkout,
// which it returns too.
func shipRoot(t *testing.T) (root, checkout string) {
	t.Helper()
	realGit(t)
	root = rootOnly(t)
	checkout = t.TempDir()
	gitCmd(t, checkout, "init", "-q")
	if code := repoCmd(root, "add", "api", "https://github.com/org/api", "--path", checkout); code != 0 {
		t.Fatal("repo add")
	}
	return root, checkout
}

// Ported from tomo TestCommitOnlyTakesWhatIsStaged: the repo is the active
// build's, not a workspace target.
func TestCommitOnlyTakesWhatIsStaged(t *testing.T) {
	root, checkout := shipRoot(t)
	if code, stderr := captureStderr(t, func() int { return runCommit(root, []string{"-m", "x"}) }); code != 1 || !strings.Contains(stderr, "need an active build") {
		t.Fatalf("no build = %d: %s", code, stderr)
	}
	seedBuild(t, root, "N-0001", "api")
	for _, f := range []string{"a.txt", "b.txt"} {
		if err := os.WriteFile(filepath.Join(checkout, f), []byte("x"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if code := runCommit(root, []string{"-m", "feat: a"}); code != 1 {
		t.Fatalf("nothing staged = %d, want 1", code)
	}
	gitCmd(t, checkout, "add", "a.txt")
	if code := runCommit(root, []string{"-m", "feat: a"}); code != 0 {
		t.Fatalf("commit = %d", code)
	}
	if files := gitCmd(t, checkout, "show", "--name-only", "--format=", "HEAD"); strings.TrimSpace(files) != "a.txt" {
		t.Fatalf("commit took = %q", files)
	}
	if status := gitCmd(t, checkout, "status", "--porcelain"); !strings.Contains(status, "?? b.txt") {
		t.Fatalf("status = %q", status)
	}
}

// call is one recorded command.
type call struct {
	dir, name string
	args      []string
}

// record swaps ship.Runner for a recorder for the test, and answers the probes
// RepoDir and PRArgs make (rev-parse, the current branch) with branch.
func record(t *testing.T, branch string) *[]call {
	t.Helper()
	runner, output := ship.Runner, ship.Output
	t.Cleanup(func() { ship.Runner, ship.Output = runner, output })
	var calls []call
	ship.Runner = func(dir, name string, args ...string) error {
		calls = append(calls, call{dir, name, args})
		return nil
	}
	ship.Output = func(dir, name string, args ...string) (string, error) {
		if len(args) > 0 && args[0] == "rev-parse" {
			return "true", nil
		}
		return branch, nil
	}
	return &calls
}

// AC8 / spec R31: pr runs gh in the build's repo, titled and linked to the ticket.
func TestPROpensFromTheBuildsRepo(t *testing.T) {
	root, checkout := shipRoot(t)
	tk, err := ticket.Parse([]byte("---\nschema: 1\nid: X-1234\ntype: task\ntitle: Fix login\nrepo: api\nlabels: []\nstatus: draft\nexternal:\n  github: org/api#1234\n---\n\n"))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "build", "tickets"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "build", "tickets", "X-1234.md"), tk.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}
	if code := runBuild(root, []string{"new", "X-1234"}); code != 0 {
		t.Fatal("build new")
	}
	if _, out := captureStdout(t, func() int { return runStatus(root) }); !strings.Contains(out, "branch:   x-1234-fix-login") {
		t.Fatalf("status doesn't suggest the branch:\n%s", out)
	}
	calls := record(t, "x-1234-fix-login")
	if code := runPR(root, nil); code != 0 {
		t.Fatalf("pr = %d", code)
	}
	if len(*calls) != 1 {
		t.Fatalf("calls = %+v", *calls)
	}
	c := (*calls)[0]
	joined := strings.Join(c.args, " ")
	if c.dir != checkout || c.name != "gh" || !strings.HasPrefix(joined, "pr create") || !strings.Contains(joined, "--title [X-1234] Fix login") {
		t.Fatalf("call = %+v", c)
	}
	if !strings.Contains(joined, "Implements X-1234: Fix login") || !strings.Contains(joined, "**github:** org/api#1234") {
		t.Fatalf("body doesn't link the ticket and its external item: %s", joined)
	}
}

// A planned ticket's PR links its plan change folder.
func TestPRLinksThePlanFolder(t *testing.T) {
	if got := planFolder(state.State{Sources: []string{"plan/specs/7-gateway/tickets/S-0036.md"}}); got != "plan/specs/7-gateway" {
		t.Fatalf("planFolder = %q", got)
	}
	if got := planFolder(state.State{Sources: []string{"build/tickets/N-0001.md"}}); got != "" {
		t.Fatalf("planFolder for an inbox ticket = %q", got)
	}
}

// AC13: with a plan change active, commit and pr refuse and run nothing.
func TestCommitAndPRNeedABuild(t *testing.T) {
	root := workspace(t)
	calls := record(t, "main")
	for name, run := range map[string]func() int{
		"commit": func() int { return runCommit(root, []string{"-m", "x"}) },
		"pr":     func() int { return runPR(root, nil) },
	} {
		if code, stderr := captureStderr(t, run); code != 1 || !strings.Contains(stderr, "commit and pr need an active build") {
			t.Errorf("%s = %d: %s", name, code, stderr)
		}
	}
	if len(*calls) != 0 {
		t.Fatalf("ran %+v", *calls)
	}
}

// D8: a build whose repo path isn't a git checkout runs nothing.
func TestPRRefusesARepoPathThatIsNotACheckout(t *testing.T) {
	realGit(t)
	root := rootOnly(t)
	if code := repoCmd(root, "add", "api", "https://github.com/org/api", "--path", t.TempDir()); code != 0 {
		t.Fatal("repo add")
	}
	seedBuild(t, root, "N-0001", "api")
	runner := ship.Runner
	t.Cleanup(func() { ship.Runner = runner })
	ran := false
	ship.Runner = func(string, string, ...string) error { ran = true; return nil }
	if code, stderr := captureStderr(t, func() int { return runPR(root, nil) }); code != 1 || !strings.Contains(stderr, "isn't a git checkout") {
		t.Fatalf("pr = %d: %s", code, stderr)
	}
	if ran {
		t.Fatal("gh ran outside a checkout")
	}
}
