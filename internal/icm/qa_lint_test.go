package icm

import (
	"strings"
	"testing"
	"testing/fstest"
)

// QA for S-0043: gaps the implement tests left.

// An AGENTS.md fragment is linted through the single-file source, and a
// missing fragment is reported rather than skipped.
func TestQALintChecksFragments(t *testing.T) {
	agents := fstest.MapFS{"build.md": &fstest.MapFile{Data: []byte("Read 05_breakdown first.\n")}}
	got := lintDocs([]lintRoom{{name: BuildRoom, owner: PlanRoom, foreign: planOnlyTerms, sources: []lintSource{
		{prefix: "scaffold/agents", fsys: agents, file: "build.md"},
		{prefix: "scaffold/agents", fsys: agents, file: "missing.md"},
	}}})
	if len(got) != 2 || got[0] != `scaffold/agents/build.md:1: plan-only term "05_breakdown" in the build room` ||
		!strings.HasPrefix(got[1], "scaffold/agents/missing.md: ") {
		t.Fatalf("problems = %q", got)
	}
}

// The embedded lint covers each room's tree and its own fragment, and no
// other: the build room is checked against plan terms and the reverse.
func TestQAScaffoldRoomsWiring(t *testing.T) {
	want := map[string]string{BuildRoom: "scaffold/build+scaffold/agents/build.md", PlanRoom: "scaffold/plan+scaffold/agents/plan.md"}
	for _, r := range scaffoldRooms() {
		var parts []string
		for _, s := range r.sources {
			p := s.prefix
			if s.file != "" {
				p += "/" + s.file
			}
			parts = append(parts, p)
		}
		if got := strings.Join(parts, "+"); got != want[r.name] {
			t.Errorf("%s room lints %s, want %s", r.name, got, want[r.name])
		}
		other := map[string][]string{BuildRoom: planOnlyTerms, PlanRoom: buildOnlyTerms}[r.name]
		if strings.Join(r.foreign, ",") != strings.Join(other, ",") || r.owner == r.name {
			t.Errorf("%s room is checked against the wrong list (owner %s)", r.name, r.owner)
		}
	}
	if len(scaffoldRooms()) != 2 {
		t.Fatalf("%d rooms linted, want 2", len(scaffoldRooms()))
	}
}

// Two terms on one line are both reported, in list order, and the output is
// the same every run (N2).
func TestQALintReportsEveryTermStably(t *testing.T) {
	files := map[string]string{
		"b.md": "The coder writes tasks.json.\n",
		"a.md": "x\nRun daiku pr.\n",
	}
	r := room(PlanRoom, BuildRoom, buildOnlyTerms, files)
	first := lintDocs([]lintRoom{r})
	want := []string{
		`scaffold/plan/a.md:2: build-only term "daiku pr" in the plan room`,
		`scaffold/plan/b.md:1: build-only term "tasks.json" in the plan room`,
		`scaffold/plan/b.md:1: build-only term "coder" in the plan room`,
	}
	if strings.Join(first, "\n") != strings.Join(want, "\n") {
		t.Fatalf("problems:\n%s\nwant:\n%s", strings.Join(first, "\n"), strings.Join(want, "\n"))
	}
	for i := 0; i < 10; i++ {
		if got := lintDocs([]lintRoom{room(PlanRoom, BuildRoom, buildOnlyTerms, files)}); strings.Join(got, "\n") != strings.Join(first, "\n") {
			t.Fatalf("run %d differs:\n%s", i, strings.Join(got, "\n"))
		}
	}
}

// Code review finding 2: a line too long to scan is reported, not skipped.
func TestLintReportsAnUnscannableFile(t *testing.T) {
	long := strings.Repeat("x", 2<<20)
	got := lintDocs([]lintRoom{room(BuildRoom, PlanRoom, planOnlyTerms, map[string]string{"big.md": long + "\nfunctional spec\n"})})
	if len(got) != 1 || !strings.HasPrefix(got[0], "scaffold/build/big.md: can't lint: ") {
		t.Fatalf("problems = %q", got)
	}
}
