package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// AC2 / spec N3 (S-0043 D7): a root-only workspace takes a pulled X- ticket,
// whose parent and requirements live in a planning workspace that isn't here,
// through tickets check, build new, every build gate, pr and ship, and never
// creates plan/. vcs is none and pr's git and gh are recorded, so nothing real
// runs.
func TestBuildOnlyEndToEnd(t *testing.T) {
	root := rootOnly(t)
	noPlan := func(step string) {
		t.Helper()
		if _, err := os.Stat(filepath.Join(root, "plan")); !os.IsNotExist(err) {
			t.Fatalf("after %s: plan/ exists (%v)", step, err)
		}
	}
	if code := repoCmd(root, "add", "api", "https://github.com/org/api", "--path", t.TempDir()); code != 0 {
		t.Fatal("repo add")
	}
	noPlan("repo add")

	draft := filepath.Join(t.TempDir(), "draft.md")
	text := "---\nschema: 1\ntype: story\ntitle: Gateway auth\nrepo: api\nparent: F-0003\nrequirements: [R1, N2]\n" +
		"labels: []\nstatus: draft\nexternal:\n  github: org/api#1234\n---\n\n## Context\nPlanned elsewhere.\n\n" +
		"## Acceptance criteria\n- [ ] AC1: Given a request, When it arrives, Then it is authorized\n"
	if err := os.WriteFile(draft, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	if code, out := captureStdout(t, func() int { return runTickets(root, []string{"pull", draft}) }); code != 0 || !strings.Contains(out, "pulled X-1234") {
		t.Fatalf("pull = %d:\n%s", code, out)
	}
	noPlan("tickets pull")
	tk := readFile(t, filepath.Join(root, "build", "tickets", "X-1234.md"))
	if !strings.Contains(tk, "parent: F-0003\n") || !strings.Contains(tk, "requirements: [R1, N2]\n") {
		t.Fatalf("the pulled ticket lost its plan references:\n%s", tk)
	}

	code, out := captureStdout(t, func() int { return runTickets(root, []string{"check"}) })
	if code != 0 || !strings.Contains(out, "! X-1234: parent F-0003 isn't in this workspace (planned elsewhere)") {
		t.Fatalf("tickets check = %d:\n%s", code, out)
	}
	noPlan("tickets check")

	if code, out := captureStdout(t, func() int { return runBuild(root, []string{"new", "X-1234"}) }); code != 0 || !strings.Contains(out, "created build X-1234 for api") {
		t.Fatalf("build new = %d:\n%s", code, out)
	}
	noPlan("build new")

	for _, g := range [][]string{
		{"ticket", "approved"}, {"requirements", "approved"}, {"design", "approved"}, {"tasks", "approved"},
		{"implement", "in-progress"}, {"implement", "complete"},
		{"test", "approved"}, {"review", "approved"}, {"security", "approved"},
		{"ship", "in-progress"},
	} {
		if code, stderr := gateStderr(t, root, g...); code != 0 {
			t.Fatalf("gate %v = %d: %s", g, code, stderr)
		}
		noPlan("gate " + strings.Join(g, " "))
	}

	calls := record(t, "x-1234-gateway-auth")
	if code, stderr := captureStderr(t, func() int { return runPR(root, nil) }); code != 0 {
		t.Fatalf("pr = %d: %s", code, stderr)
	}
	sawGH := false
	for _, c := range *calls {
		sawGH = sawGH || c.name == "gh"
	}
	if !sawGH {
		t.Fatalf("pr ran no gh: %+v", *calls)
	}
	noPlan("pr")

	if code, stderr := gateStderr(t, root, "ship", "complete"); code != 0 {
		t.Fatalf("ship complete = %d: %s", code, stderr)
	}
	noPlan("ship complete")

	_, out = captureStdout(t, func() int { return runStatus(root, nil) })
	if !strings.Contains(out, "  api  free\n  * X-1234  ship complete (done)\n") {
		t.Fatalf("board after ship:\n%s", out)
	}
}
