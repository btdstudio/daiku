package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/repos"
	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/state"
)

// The status board (S-0042 R1–R4, D6, D8).

// setPhase records c at phase and status, without the loop's checks, and makes
// it active.
func setPhase(t *testing.T, root, name, phase, status string) {
	t.Helper()
	changes, err := state.ReadChanges(root)
	if err != nil {
		t.Fatal(err)
	}
	c := changes[name]
	c.Phase, c.Status = phase, status
	c.Gates[phase] = status
	if err := state.SaveState(root, c); err != nil {
		t.Fatal(err)
	}
}

// boardBuild records build id on repo from an inbox ticket whose change is
// plan (none when "").
func boardBuild(t *testing.T, root, id, repo, plan string) {
	t.Helper()
	tk, err := ticket.New(ticket.Ticket{ID: id, Type: ticket.Task, Title: "Fix login", Repo: repo, Change: plan, Body: "\n"})
	if err != nil {
		t.Fatal(err)
	}
	rel := "build/tickets/" + id + ".md"
	if err := os.MkdirAll(filepath.Join(root, "build", "tickets"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, filepath.FromSlash(rel)), tk.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := state.SeedBuild(root, id, repo, rel); err != nil {
		t.Fatal(err)
	}
}

// boardRoot is two plans and three builds on api and web, with S-0036 active:
// S-0036 holds api, S-0030 has shipped, N-0040 is on web.
func boardRoot(t *testing.T) string {
	t.Helper()
	root := buildRoot(t)
	for _, name := range []string{"7-gateway", "8-billing"} {
		if _, err := state.SeedChange(root, name, false, nil); err != nil {
			t.Fatal(err)
		}
	}
	setPhase(t, root, "7-gateway", "breakdown", "approved")
	setPhase(t, root, "8-billing", "spec", "draft")
	boardBuild(t, root, "S-0030", "api", "7-gateway")
	setPhase(t, root, "S-0030", "ship", "complete")
	boardBuild(t, root, "N-0040", "web", "")
	setPhase(t, root, "N-0040", "design", "draft")
	boardBuild(t, root, "S-0036", "api", "7-gateway")
	setPhase(t, root, "S-0036", "implement", "in-progress")
	return root
}

func apiPath(t *testing.T, root string) string {
	t.Helper()
	reg, err := repos.Load(root, "daiku")
	if err != nil {
		t.Fatal(err)
	}
	p, err := reg.Resolve("api")
	if err != nil {
		t.Fatal(err)
	}
	return p
}

// AC1: the active block, then plans, then builds by repo, with the active one
// marked and each repo's holder.
func TestStatusBoard(t *testing.T) {
	root := boardRoot(t)
	code, out := captureStdout(t, func() int { return runStatus(root, nil) })
	want := "change:   S-0036\n" +
		"room:     build\n" +
		"repo:     api\n" +
		"phase:    implement\n" +
		"status:   in-progress\n" +
		"gates:    ticket=draft requirements= design= tasks= implement=in-progress test= review= security= ship=\n" +
		"initiative: 7-gateway\n" +
		"code:     api=" + apiPath(t, root) + "\n" +
		"ticket:   S-0036 Fix login\n" +
		"branch:   s-0036-fix-login\n" +
		"vcs:      none\n" +
		"\n" +
		"plans:\n" +
		"  7-gateway  breakdown approved\n" +
		"  8-billing  spec draft\n" +
		"\n" +
		"builds:\n" +
		"  api  held by S-0036\n" +
		"    S-0030  ship complete (done)\n" +
		"  * S-0036  implement in-progress\n" +
		"  web  free\n" +
		"    N-0040  design draft\n"
	if code != 0 || out != want {
		t.Fatalf("status = %d:\n%s\nwant:\n%s", code, out, want)
	}
}

// AC5: a shipped build is done and holds nothing; closed, it's gone. A closed
// plan isn't listed either.
func TestStatusBoardShippedAndClosed(t *testing.T) {
	root := boardRoot(t)
	setPhase(t, root, "S-0036", "tasks", "approved")
	_, out := captureStdout(t, func() int { return runStatus(root, nil) })
	if !strings.Contains(out, "  api  free\n    S-0030  ship complete (done)\n") {
		t.Fatalf("shipped S-0030 holds api:\n%s", out)
	}
	for _, name := range []string{"S-0030", "8-billing"} {
		if code := runChange(root, []string{"close", name, "--reason", "done"}); code != 0 {
			t.Fatalf("close %s", name)
		}
	}
	_, out = captureStdout(t, func() int { return runStatus(root, nil) })
	if strings.Contains(out, "S-0030") || strings.Contains(out, "8-billing") {
		t.Fatalf("closed changes listed:\n%s", out)
	}
}

// AC9 and R4: no changes at all, and changes with none active.
func TestStatusWithoutAnActiveChange(t *testing.T) {
	root := buildRoot(t)
	code, out := captureStdout(t, func() int { return runStatus(root, nil) })
	if code != 0 || out != "no active change\nvcs:      none\n\nno changes yet\n" {
		t.Fatalf("empty status = %d: %q", code, out)
	}
	if _, err := state.SeedChange(root, "7-gateway", false, nil); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(state.ActivePath(root)); err != nil {
		t.Fatal(err)
	}
	code, out = captureStdout(t, func() int { return runStatus(root, nil) })
	if code != 0 || out != "no active change\nvcs:      none\n\nplans:\n  7-gateway  intake draft\n" {
		t.Fatalf("status = %d: %q", code, out)
	}
}

// R2: a plan's initiative is itself and its code paths list every repo; a
// build whose plan isn't here says so; a repo with no path is "(not set)".
func TestStatusInitiativeAndCode(t *testing.T) {
	root := buildRoot(t)
	if _, err := state.SeedChange(root, "7-gateway", true, nil, "api", "web"); err != nil {
		t.Fatal(err)
	}
	_, out := captureStdout(t, func() int { return runStatus(root, nil) })
	for _, want := range []string{"repo:     api, web\n", "initiative: 7-gateway\n", "code:     api=" + apiPath(t, root) + " web="} {
		if !strings.Contains(out, want) {
			t.Errorf("plan status lacks %q:\n%s", want, out)
		}
	}
	if code := repoCmd(root, "add", "ops", "https://github.com/org/ops"); code != 0 {
		t.Fatal("repo add ops")
	}
	boardBuild(t, root, "X-1234", "ops", "9-elsewhere")
	_, out = captureStdout(t, func() int { return runStatus(root, nil) })
	for _, want := range []string{"initiative: 9-elsewhere (not in this workspace)\n", "code:     ops=(not set)\n"} {
		if !strings.Contains(out, want) {
			t.Errorf("build status lacks %q:\n%s", want, out)
		}
	}
}
