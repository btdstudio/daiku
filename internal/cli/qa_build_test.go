package cli

import (
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku"
)

// QA (AC10, build side): the build room's analyst fetches through the one root
// sync.md, and the build room ships no sync.md of its own.
func TestQABuildAnalystUsesTheRootSyncFile(t *testing.T) {
	analyst, err := fs.ReadFile(daiku.BuildFS, "01_design/personas/analyst.md")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(strings.Join(strings.Fields(string(analyst)), " "), "the workspace's `sync.md` (**fetch**)") {
		t.Errorf("the build analyst doesn't point at the workspace's sync.md:\n%s", analyst)
	}
	err = fs.WalkDir(daiku.BuildFS, ".", func(p string, d fs.DirEntry, err error) error {
		if err == nil && filepath.Base(p) == "sync.md" {
			t.Errorf("the build room ships its own %s", p)
		}
		return err
	})
	if err != nil {
		t.Fatal(err)
	}
}

// QA (AC7 / spec R29): closing the build that holds a repo frees it.
func TestQAClosingTheHolderFreesTheRepo(t *testing.T) {
	root := buildRoot(t)
	seedBuild(t, root, "S-0036", "api")
	gateAll(t, root, append(throughTasks, []string{"implement", "in-progress"})...)
	seedBuild(t, root, "N-0040", "api")
	gateAll(t, root, throughTasks...)
	if code, _ := gateStderr(t, root, "implement", "in-progress"); code != 1 {
		t.Fatal("the repo wasn't locked")
	}
	if code := runChange(root, []string{"close", "S-0036", "--reason", "abandoned"}); code != 0 {
		t.Fatal("close S-0036")
	}
	gateAll(t, root, []string{"implement", "in-progress"})
	if log := readLog(t, root); strings.Contains(log, "override") {
		t.Fatalf("freeing by close needed an override:\n%s", log)
	}
}

// QA (AC12, build gate): with LOG.md a symlink, a build gate refuses and changes
// nothing: STATE.json, the log's target and the gate stay as they were.
func TestQABuildGateRefusesASymlinkedLog(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	root := buildRoot(t)
	seedBuild(t, root, "N-0001", "api")
	logPath := filepath.Join(root, "LOG.md")
	victim := filepath.Join(t.TempDir(), "victim")
	if err := os.WriteFile(victim, []byte(readLog(t, root)), 0o644); err != nil {
		t.Fatal(err)
	}
	logBefore := readFile(t, victim)
	os.Remove(logPath)
	if err := os.Symlink(victim, logPath); err != nil {
		t.Fatal(err)
	}
	stateBefore := readFile(t, filepath.Join(root, "STATE.json"))
	code, stderr := gateStderr(t, root, "ticket", "approved")
	if code != 1 || !strings.Contains(stderr, "LOG.md is a symlink") {
		t.Fatalf("gate = %d: %s", code, stderr)
	}
	if readFile(t, filepath.Join(root, "STATE.json")) != stateBefore || readFile(t, victim) != logBefore {
		t.Fatal("a refused build gate changed STATE.json or the log's target")
	}
}
