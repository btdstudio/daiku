package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// QA for S-0042: gaps the implement tests left.

// N2: neither status view runs git, even in a git workspace. A fake git that
// records every call is the only git on PATH.
func TestQAStatusViewsRunNoGit(t *testing.T) {
	root := initiativeRoot(t)
	if err := os.WriteFile(filepath.Join(root, ".daiku", "settings.json"), []byte(`{"vcs": "git"}`), 0o644); err != nil {
		t.Fatal(err)
	}
	marker := fakeGit(t)
	for _, args := range [][]string{nil, {"7-gateway"}, {"S-0036"}} {
		captureStderr(t, func() int {
			code, _ := captureStdout(t, func() int { return runStatus(root, args) })
			return code
		})
	}
	if calls, err := os.ReadFile(marker); err == nil {
		t.Fatalf("status ran git: %s", calls)
	}
}

// N4: the board and the initiative view print the same bytes every run, so map
// order never leaks into the output.
func TestQAStatusOutputIsStable(t *testing.T) {
	root := boardRoot(t)
	for _, args := range [][]string{nil, {"7-gateway"}} {
		_, first := captureStdout(t, func() int { return runStatus(root, args) })
		for i := 0; i < 20; i++ {
			if _, out := captureStdout(t, func() int { return runStatus(root, args) }); out != first {
				t.Fatalf("status %v run %d differs:\n%s\nfirst:\n%s", args, i, out, first)
			}
		}
	}
}

// N5 / AC7: under git, reopen and close of a build commit its plan's README
// too; the implement tests checked build new and a gate.
func TestQAReopenAndCloseCommitThePlansOverview(t *testing.T) {
	root := plannedBuild(t, true)
	gateAll(t, root, throughTasks...)
	if code, stderr := captureStderr(t, func() int { return runReopen(root, []string{"design", "--reason", "rethink"}) }); code != 0 {
		t.Fatalf("reopen = %d: %s", code, stderr)
	}
	inHead(t, root, "reopen", authReadme)
	if got := buildsRow(t, root); !strings.Contains(got, "| design draft |") {
		t.Fatalf("after reopen: %s", got)
	}
	if code, stderr := captureStderr(t, func() int { return runChange(root, []string{"close", "S-0001", "--reason", "dropped"}) }); code != 0 {
		t.Fatalf("close = %d: %s", code, stderr)
	}
	inHead(t, root, "close", authReadme)
	if got := buildsRow(t, root); !strings.Contains(got, "| closed |") {
		t.Fatalf("after close: %s", got)
	}
	current(t, root, "after close")
}

// R9: the plan's own transition keeps the build section as a build wrote it.
func TestQAPlanTransitionKeepsTheBuildsSection(t *testing.T) {
	root := plannedBuild(t, false)
	gateAll(t, root, []string{"ticket", "approved"})
	byBuild := buildsRow(t, root)
	if code := runSwitch(root, []string{"1-auth-flow"}); code != 0 {
		t.Fatal("switch to the plan")
	}
	if code, stderr := captureStderr(t, func() int { return runReopen(root, []string{"breakdown", "--reason", "one more ticket"}) }); code != 0 {
		t.Fatalf("plan reopen = %d: %s", code, stderr)
	}
	if byPlan := buildsRow(t, root); byPlan != byBuild {
		t.Fatalf("plan transition rewrote the row:\n%s\nbuild wrote:\n%s", byPlan, byBuild)
	}
	current(t, root, "after the plan's reopen")
}
