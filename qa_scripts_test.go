package daiku_test

import (
	"io"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

// QA for S-0043: the cold-start script's edges.

// COLDSTART_KEEP=1 keeps the workspaces and says where, and a TMPDIR with a
// space in it works.
func TestQAColdstartKeepAndSpaces(t *testing.T) {
	tmp := filepath.Join(t.TempDir(), "with space")
	if err := os.Mkdir(tmp, 0o755); err != nil {
		t.Fatal(err)
	}
	cmd, stdin, lines, _, _ := coldstartEnv(t, "TMPDIR="+tmp, "COLDSTART_KEEP=1")
	var out strings.Builder
	ws := nextWorkspace(t, lines, &out)
	if !strings.Contains(ws, "with space") {
		t.Fatalf("workspace %q isn't under the TMPDIR with a space", ws)
	}
	io.WriteString(stdin, "\n")
	nextWorkspace(t, lines, &out)
	io.WriteString(stdin, "\n")
	finish(t, cmd, stdin, lines, &out)
	m := regexp.MustCompile(`(?m)^kept: (.+)$`).FindStringSubmatch(out.String())
	if m == nil {
		t.Fatalf("no kept line:\n%s", out.String())
	}
	if _, err := os.Stat(filepath.Join(m[1], "build-ws", "STATE.json")); err != nil {
		t.Fatalf("kept folder lacks the build workspace: %v", err)
	}
}

// N4: the script never pushes, tags, publishes or runs an agent: the only git
// it runs is a local init, and it calls no gh.
func TestQAColdstartRunsNothingOutward(t *testing.T) {
	data, err := os.ReadFile("scripts/coldstart.sh")
	if err != nil {
		t.Fatal(err)
	}
	for _, l := range strings.Split(string(data), "\n") {
		code := strings.TrimSpace(l)
		if strings.HasPrefix(code, "#") {
			continue
		}
		for _, bad := range []string{"git push", "git tag", "gh ", "curl", "wget", "claude", "codex"} {
			if strings.Contains(code, bad) {
				t.Errorf("script line runs %q: %s", bad, l)
			}
		}
		if strings.Contains(code, "git ") && !strings.Contains(code, "git init -q") {
			t.Errorf("script runs git beyond a local init: %s", l)
		}
	}
}
