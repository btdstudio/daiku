package cli

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func entries(t *testing.T, dir string) int {
	t.Helper()
	list, _ := os.ReadDir(dir)
	return len(list)
}

// QA (AC10): change new through a symlinked plan/ or plan/specs is refused,
// names the link, and leaves nothing outside the workspace or in STATE.json.
func TestQAChangeNewRefusesSymlinkedPlanFolders(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	t.Run("plan", func(t *testing.T) {
		root := rootOnly(t)
		outside := t.TempDir()
		if err := os.Symlink(outside, filepath.Join(root, "plan")); err != nil {
			t.Fatal(err)
		}
		code, stderr := captureStderr(t, func() int { return runChange(root, []string{"new", "Gateway"}) })
		if code != 1 || !strings.Contains(stderr, "is a symlink") {
			t.Fatalf("change new = %d, stderr %q", code, stderr)
		}
		if n := entries(t, outside); n != 0 {
			t.Fatalf("wrote %d entries outside the workspace", n)
		}
		if state, _ := os.ReadFile(filepath.Join(root, "STATE.json")); strings.Contains(string(state), "gateway") {
			t.Fatalf("a refused change was recorded:\n%s", state)
		}
	})
	t.Run("plan/specs", func(t *testing.T) {
		root := rootOnly(t)
		if code := runChange(root, []string{"new", "First"}); code != 0 {
			t.Fatal("first change new")
		}
		outside := t.TempDir()
		specs := filepath.Join(root, "plan", "specs")
		if err := os.RemoveAll(specs); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, specs); err != nil {
			t.Fatal(err)
		}
		code, stderr := captureStderr(t, func() int { return runChange(root, []string{"new", "Gateway"}) })
		if code != 1 || !strings.Contains(stderr, "plan/specs is a symlink") {
			t.Fatalf("change new = %d, stderr %q", code, stderr)
		}
		if n := entries(t, outside); n != 0 {
			t.Fatalf("wrote %d entries outside the workspace", n)
		}
		// The intake written before the refusal is removed again.
		if ids := intakeIDs(t, root); len(ids) != 1 || ids[0] != "1.md" {
			t.Fatalf("intake files = %v, want only the first change's", ids)
		}
	})
}

// QA (AC8): in a zumen workspace, change new and gate also refuse, naming zumen,
// and write nothing.
func TestQAZumenWorkspaceRefusesWrites(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".zumen"), 0o755); err != nil {
		t.Fatal(err)
	}
	for name, run := range map[string]func() int{
		"change new": func() int { return runChange(root, []string{"new", "Gateway"}) },
		"gate":       func() int { return runGate(root, []string{"intake", "approved"}) },
		"switch":     func() int { return runSwitch(root, nil) },
	} {
		code, stderr := captureStderr(t, run)
		if code != 1 || !strings.Contains(stderr, "a zumen workspace") {
			t.Errorf("%s = %d, stderr %q", name, code, stderr)
		}
	}
	if n := entries(t, root); n != 1 {
		t.Fatalf("wrote %d entries in a zumen workspace", n-1)
	}
}
