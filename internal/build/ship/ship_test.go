package ship

import (
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/repos"
	"github.com/btdstudio/daiku/internal/core/ticket"
)

const plannedTicket = `---
schema: 1
id: S-0014
type: story
title: Add release workflow
repo: app
change: 3-release
spec: plan/specs/3-release/functional-spec.md
adrs: [ADR-0004]
labels: []
status: ready
---

## Acceptance criteria
- [ ] AC1: Given a tag, When it's pushed, Then a release is built.
`

func published(t *testing.T) *ticket.Ticket {
	t.Helper()
	tk, err := ticket.Parse([]byte(plannedTicket))
	if err != nil {
		t.Fatal(err)
	}
	if err := tk.MarkPublished("github", "org/app#7"); err != nil {
		t.Fatal(err)
	}
	return tk
}

// Ported from tomo TestPublishedTicketImportsFrozenAndPRLinksIt (its PR part):
// the import is gone, since daiku reads the ticket in place.
func TestPRBodyAndArgsLinkTheTicket(t *testing.T) {
	tk := published(t)
	body := PRBody(tk, "plan/specs/3-release")
	for _, want := range []string{"Implements S-0014: Add release workflow", "**github:** org/app#7", "**Plan:** `plan/specs/3-release/`", "**Spec:** `plan/specs/3-release/functional-spec.md`", "ADR-0004"} {
		if !strings.Contains(body, want) {
			t.Errorf("body missing %q:\n%s", want, body)
		}
	}
	t.Cleanup(restore())
	Output = func(string, string, ...string) (string, error) { return "s-0014-add-release-workflow", nil }
	joined := strings.Join(PRArgs(t.TempDir(), tk, "plan/specs/3-release", nil), " ")
	if !strings.Contains(joined, "--title [S-0014] Add release workflow") || !strings.Contains(joined, "--body") {
		t.Fatalf("args = %s", joined)
	}
	if args := PRArgs(t.TempDir(), tk, "", []string{"My title"}); !strings.Contains(strings.Join(args, " "), "--title My title") {
		t.Fatalf("args = %v", args)
	}
	if args := PRArgs(t.TempDir(), nil, "", nil); strings.Join(args, " ") != "pr create" {
		t.Fatalf("args without a ticket = %v", args)
	}
}

// restore puts the real Runner and Output back.
func restore() func() {
	r, o := Runner, Output
	return func() { Runner, Output = r, o }
}

func TestPRBodyWithoutAPlan(t *testing.T) {
	tk, err := ticket.New(ticket.Ticket{ID: "N-0001", Type: ticket.Task, Title: "Fix login", Repo: "api"})
	if err != nil {
		t.Fatal(err)
	}
	if body := PRBody(tk, ""); body != "Implements N-0001: Fix login\n" {
		t.Fatalf("body = %q", body)
	}
}

func TestBranchName(t *testing.T) {
	for id, want := range map[string]string{"S-0036": "s-0036-add-release-workflow", "X-PROJ-12": "x-proj-12-add-release-workflow"} {
		tk, err := ticket.New(ticket.Ticket{ID: id, Type: ticket.Story, Title: "Add release workflow!", Repo: "app"})
		if err != nil {
			t.Fatal(err)
		}
		if got := BranchName(tk); got != want {
			t.Errorf("BranchName(%s) = %q, want %q", id, got, want)
		}
	}
}

func TestPRArgsWarnsAboutABranchWithoutTheID(t *testing.T) {
	t.Cleanup(restore())
	Output = func(string, string, ...string) (string, error) { return "main", nil }
	stderr := captureStderr(t, func() { PRArgs(t.TempDir(), published(t), "", nil) })
	if !strings.Contains(stderr, "warning: branch main doesn't include S-0014") {
		t.Fatalf("stderr = %q", stderr)
	}
}

func captureStderr(t *testing.T, fn func()) string {
	t.Helper()
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	old := os.Stderr
	os.Stderr = w
	fn()
	w.Close()
	os.Stderr = old
	data, _ := io.ReadAll(r)
	return string(data)
}

// registered makes a workspace with repo app at path, which may not exist.
func registered(t *testing.T, path string) string {
	t.Helper()
	root := t.TempDir()
	reg, err := repos.Load(root, "daiku")
	if err != nil {
		t.Fatal(err)
	}
	if err := reg.Add("app", "https://github.com/org/app", ""); err != nil {
		t.Fatal(err)
	}
	if err := reg.SetPath("app", path); err != nil {
		t.Fatal(err)
	}
	if err := reg.Save(); err != nil {
		t.Fatal(err)
	}
	return root
}

// D8 / N4: commit and pr only run in a registered git checkout.
func TestRepoDir(t *testing.T) {
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not on PATH")
	}
	t.Setenv("GIT_CONFIG_GLOBAL", os.DevNull)
	t.Setenv("GIT_CONFIG_NOSYSTEM", "1")
	checkout := t.TempDir()
	if out, err := exec.Command("git", "-C", checkout, "init", "-q").CombinedOutput(); err != nil {
		t.Fatalf("git init: %v %s", err, out)
	}
	if dir, err := RepoDir(registered(t, checkout), "app"); err != nil || dir != checkout {
		t.Fatalf("RepoDir = %q, %v", dir, err)
	}
	file := filepath.Join(t.TempDir(), "file")
	os.WriteFile(file, nil, 0o644)
	for name, path := range map[string]string{
		"missing":   filepath.Join(t.TempDir(), "gone"),
		"a file":    file,
		"not a git": t.TempDir(),
	} {
		if _, err := RepoDir(registered(t, path), "app"); err == nil || !strings.Contains(err.Error(), "daiku repo path app") {
			t.Errorf("%s: err = %v", name, err)
		}
	}
	if _, err := RepoDir(registered(t, checkout), "web"); err == nil || !strings.Contains(err.Error(), "unknown repo") {
		t.Errorf("unregistered: err = %v", err)
	}
}
