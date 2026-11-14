package daiku_test

import (
	"bufio"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"
)

// scripts/coldstart.sh (S-0043 D8, D9): the test plays the agent. It reads the
// script's output to each "workspace:" line, acts there (or doesn't), and
// presses Enter.

// coldstart builds daiku and starts the script with DAIKU set and its own
// TMPDIR, so the test can see what it leaves behind.
func coldstart(t *testing.T) (cmd *exec.Cmd, stdin io.WriteCloser, lines *bufio.Scanner, daiku, tmp string) {
	t.Helper()
	return coldstartEnv(t)
}

// coldstartEnv is coldstart with extra environment entries, which override the
// defaults (the last entry for a key wins).
func coldstartEnv(t *testing.T, extra ...string) (cmd *exec.Cmd, stdin io.WriteCloser, lines *bufio.Scanner, daiku, tmp string) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("a POSIX sh script")
	}
	for _, tool := range []string{"sh", "git", "go"} {
		if _, err := exec.LookPath(tool); err != nil {
			t.Skipf("%s not on PATH", tool)
		}
	}
	daiku = filepath.Join(t.TempDir(), "daiku")
	build := exec.Command("go", "build", "-o", daiku, "./cmd/daiku")
	build.Env = append(os.Environ(), "GOWORK=off")
	if out, err := build.CombinedOutput(); err != nil {
		t.Fatalf("go build: %v\n%s", err, out)
	}
	tmp = t.TempDir()
	cmd = exec.Command("sh", "scripts/coldstart.sh")
	cmd.Env = append(append(os.Environ(), "DAIKU="+daiku, "TMPDIR="+tmp, "COLDSTART_KEEP="), extra...)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		t.Fatal(err)
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	cmd.Stderr = cmd.Stdout
	if err := cmd.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { cmd.Process.Kill() })
	return cmd, stdin, bufio.NewScanner(stdout), daiku, tmp
}

// nextWorkspace reads output up to the next "press Enter" line, where the
// script waits, and returns the workspace it named, keeping what it read in out.
func nextWorkspace(t *testing.T, lines *bufio.Scanner, out *strings.Builder) string {
	t.Helper()
	ws := ""
	for lines.Scan() {
		out.WriteString(lines.Text() + "\n")
		if p, ok := strings.CutPrefix(lines.Text(), "workspace: "); ok {
			ws = p
		}
		if strings.Contains(lines.Text(), "press Enter") {
			if ws == "" {
				t.Fatalf("the script waits without naming a workspace:\n%s", out)
			}
			return ws
		}
	}
	t.Fatalf("the script didn't wait for Enter:\n%s", out)
	return ""
}

// finish presses Enter, reads the rest and waits, with a deadline.
func finish(t *testing.T, cmd *exec.Cmd, stdin io.WriteCloser, lines *bufio.Scanner, out *strings.Builder) error {
	t.Helper()
	stdin.Close()
	for lines.Scan() {
		out.WriteString(lines.Text() + "\n")
	}
	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()
	select {
	case err := <-done:
		return err
	case <-time.After(60 * time.Second):
		t.Fatalf("the script didn't finish:\n%s", out)
		return nil
	}
}

// leftovers lists what the script left in its TMPDIR.
func leftovers(t *testing.T, tmp string) []string {
	t.Helper()
	entries, err := os.ReadDir(tmp)
	if err != nil {
		t.Fatal(err)
	}
	var names []string
	for _, e := range entries {
		names = append(names, e.Name())
	}
	return names
}

// AC4: when the agent does what each prompt asks, both pass, and the temp
// workspaces are removed.
func TestColdstartPasses(t *testing.T) {
	cmd, stdin, lines, daiku, tmp := coldstart(t)
	var out strings.Builder
	act := func(ws string, args ...string) {
		t.Helper()
		c := exec.Command(daiku, args...)
		c.Dir = ws
		if b, err := c.CombinedOutput(); err != nil {
			t.Fatalf("daiku %v in %s: %v\n%s", args, ws, err, b)
		}
		io.WriteString(stdin, "\n")
	}
	ws := nextWorkspace(t, lines, &out)
	if !strings.Contains(out.String(), "I have a ticket, help me build it") {
		t.Fatalf("the build prompt wasn't shown:\n%s", out.String())
	}
	act(ws, "build", "new", "N-0001")
	ws = nextWorkspace(t, lines, &out)
	if !strings.Contains(out.String(), "Help me plan a password reset flow") {
		t.Fatalf("the plan prompt wasn't shown:\n%s", out.String())
	}
	act(ws, "change", "new", "Password reset")
	if err := finish(t, cmd, stdin, lines, &out); err != nil {
		t.Fatalf("script failed: %v\n%s", err, out.String())
	}
	for _, want := range []string{"PASS build\n", "PASS plan\n", "cold start: both prompts passed\n"} {
		if !strings.Contains(out.String(), want) {
			t.Errorf("output lacks %q:\n%s", want, out.String())
		}
	}
	if left := leftovers(t, tmp); len(left) != 0 {
		t.Fatalf("the script left %v in its temp folder", left)
	}
}

// AC7: when the agent does nothing, the script fails naming the prompt, and
// still removes its temp workspaces.
func TestColdstartFailsWhenNothingHappens(t *testing.T) {
	cmd, stdin, lines, _, tmp := coldstart(t)
	var out strings.Builder
	nextWorkspace(t, lines, &out)
	io.WriteString(stdin, "\n")
	nextWorkspace(t, lines, &out)
	io.WriteString(stdin, "\n")
	err := finish(t, cmd, stdin, lines, &out)
	if exit, ok := err.(*exec.ExitError); !ok || exit.ExitCode() == 0 {
		t.Fatalf("exit = %v, want non-zero:\n%s", err, out.String())
	}
	for _, want := range []string{"FAIL build: no build change for ticket N-0001", "FAIL plan:", "cold start: failed\n"} {
		if !strings.Contains(out.String(), want) {
			t.Errorf("output lacks %q:\n%s", want, out.String())
		}
	}
	if left := leftovers(t, tmp); len(left) != 0 {
		t.Fatalf("the script left %v in its temp folder", left)
	}
}
