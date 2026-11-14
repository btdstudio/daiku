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

// The initiative view (S-0042 R5, R6, D7).

// initiativeRoot is plan 7-gateway with tickets S-0036 and S-0037 on api, and
// build S-0036 at implement.
func initiativeRoot(t *testing.T) string {
	t.Helper()
	root := buildRoot(t)
	if _, err := state.SeedChange(root, "7-gateway", false, nil); err != nil {
		t.Fatal(err)
	}
	setPhase(t, root, "7-gateway", "breakdown", "approved")
	for _, id := range []string{"S-0036", "S-0037"} {
		tk, err := ticket.New(ticket.Ticket{ID: id, Type: ticket.Story, Title: "Gateway " + id, Repo: "api", Change: "7-gateway", Body: "\n"})
		if err != nil {
			t.Fatal(err)
		}
		p := filepath.Join(root, "plan", "specs", "7-gateway", "tickets", id+".md")
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, tk.Bytes(), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := state.SeedBuild(root, "S-0036", "api", "plan/specs/7-gateway/tickets/S-0036.md"); err != nil {
		t.Fatal(err)
	}
	setPhase(t, root, "S-0036", "implement", "in-progress")
	return root
}

// AC2: each ticket with its repo and build, or not started.
func TestStatusOfAPlan(t *testing.T) {
	root := initiativeRoot(t)
	code, out := captureStdout(t, func() int { return runStatus(root, []string{"7-gateway"}) })
	want := "7-gateway  breakdown approved\n" +
		"  S-0036  api  implement in-progress\n" +
		"  S-0037  api  not started\n"
	if code != 0 || out != want {
		t.Fatalf("status 7-gateway = %d:\n%s\nwant:\n%s", code, out, want)
	}
}

// AC6: a plan that isn't here, a build, and an unknown name are refused.
func TestStatusOfANonPlanIsRefused(t *testing.T) {
	root := buildRoot(t)
	boardBuild(t, root, "S-0036", "api", "7-gateway")
	for name, want := range map[string]string{
		"7-gateway": "daiku: 7-gateway isn't a plan change in this workspace\n",
		"S-0036":    "daiku: S-0036 isn't a plan change in this workspace (it's a build — bare daiku status lists it)\n",
		"../etc":    "daiku: ../etc isn't a plan change in this workspace\n",
	} {
		code, stderr := captureStderr(t, func() int { return runStatus(root, []string{name}) })
		if code != 1 || stderr != want {
			t.Errorf("status %s = %d: %q", name, code, stderr)
		}
	}
	if code, stderr := captureStderr(t, func() int { return runStatus(root, []string{"a", "b"}) }); code != 1 || !strings.Contains(stderr, "daiku status [<plan-change>]") {
		t.Errorf("two args = %d: %q", code, stderr)
	}
}

// treeDigest is every path and its contents under root.
func treeDigest(t *testing.T, root string) string {
	t.Helper()
	var b strings.Builder
	err := filepath.WalkDir(root, func(p string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		b.WriteString(p + "\n")
		if d.Type().IsRegular() {
			data, err := os.ReadFile(p)
			if err != nil {
				return err
			}
			b.Write(data)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return b.String()
}

// AC10 / N2: neither view changes a file.
func TestStatusViewsWriteNothing(t *testing.T) {
	root := initiativeRoot(t)
	before := treeDigest(t, root)
	for _, args := range [][]string{nil, {"7-gateway"}, {"S-0036"}} {
		captureStdout(t, func() int { return runStatus(root, args) })
		if after := treeDigest(t, root); after != before {
			t.Fatalf("status %v changed the workspace", args)
		}
	}
}
