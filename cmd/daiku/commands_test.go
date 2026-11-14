package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// captureBoth runs fn and returns its exit code, stdout and stderr.
func captureBoth(t *testing.T, fn func() int) (int, string, string) {
	t.Helper()
	var errOut string
	code, out := capture(t, func() int {
		r, w, err := os.Pipe()
		if err != nil {
			t.Fatal(err)
		}
		orig := os.Stderr
		os.Stderr = w
		c := fn()
		os.Stderr = orig
		w.Close()
		buf := make([]byte, 1<<16)
		n, _ := r.Read(buf)
		errOut = string(buf[:n])
		return c
	})
	return code, out, errOut
}

func TestVersion(t *testing.T) {
	code, out := capture(t, func() int { return run([]string{"version"}) })
	if code != 0 || out != "daiku dev\n" {
		t.Fatalf("version = %d, %q", code, out)
	}
}

// AC1: daiku init in an empty folder writes the root and no room.
func TestInitIsRootOnly(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	dir := filepath.Join(t.TempDir(), "ws")
	code, out := capture(t, func() int { return run([]string{"init", dir, "--no-git"}) })
	if code != 0 || !strings.Contains(out, "scaffolded daiku workspace in "+dir) {
		t.Fatalf("init = %d:\n%s", code, out)
	}
	for _, rel := range []string{"AGENTS.md", "CLAUDE.md", "README.md", "STATE.json", "LOG.md", "REPOS.json", ".daiku", "ideas", "references"} {
		if _, err := os.Stat(filepath.Join(dir, rel)); err != nil {
			t.Errorf("init didn't write %s", rel)
		}
	}
	for _, rel := range []string{"plan", "build"} {
		if _, err := os.Stat(filepath.Join(dir, rel)); err == nil {
			t.Errorf("init wrote %s/", rel)
		}
	}
}

// AC9: daiku provides no zumen or tomo command.
func TestNoZumenOrTomo(t *testing.T) {
	for _, name := range []string{"zumen", "tomo"} {
		code, out, errOut := captureBoth(t, func() int { return run([]string{name}) })
		if code != 1 || out != "" || !strings.Contains(errOut, `daiku: unknown command "`+name+`"`) {
			t.Errorf("%s = %d, stdout %q, stderr %q", name, code, out, errOut)
		}
	}
}

// No arguments prints usage to stderr and fails, as zumen did; help succeeds.
func TestUsageAndHelp(t *testing.T) {
	code, out, errOut := captureBoth(t, func() int { return run(nil) })
	if code != 1 || out != "" || !strings.Contains(errOut, "Usage:") {
		t.Errorf("no args = %d, stdout %q, stderr %q", code, out, errOut)
	}
	code, out = capture(t, func() int { return run([]string{"help"}) })
	if code != 0 || !strings.Contains(out, "daiku init") || !strings.Contains(out, "daiku change new") {
		t.Errorf("help = %d:\n%s", code, out)
	}
}

// init refuses a zumen workspace and writes nothing.
func TestInitRefusesAZumenWorkspace(t *testing.T) {
	dir := t.TempDir()
	if err := os.Mkdir(filepath.Join(dir, ".zumen"), 0o755); err != nil {
		t.Fatal(err)
	}
	code, _, errOut := captureBoth(t, func() int { return run([]string{"init", dir, "--no-git"}) })
	if code != 1 || !strings.Contains(errOut, "a zumen workspace") {
		t.Fatalf("init = %d, stderr %q", code, errOut)
	}
	if entries, _ := os.ReadDir(dir); len(entries) != 1 {
		t.Fatalf("init wrote %d entries", len(entries)-1)
	}
}

// init over an AGENTS.md it can't merge says so and exits 0.
func TestInitReportsAnUnmergeableAgents(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte("mine\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	code, out := capture(t, func() int { return run([]string{"init", dir, "--no-git"}) })
	if code != 0 || !strings.Contains(out, "skipped (edited) AGENTS.md: can't merge: no ## headings") {
		t.Fatalf("init = %d:\n%s", code, out)
	}
}
