package icm

import (
	"io/fs"
	"strings"
	"testing"
	"testing/fstest"
)

// room is a lintRoom over one in-memory tree.
func room(name, owner string, foreign []string, files map[string]string) lintRoom {
	m := fstest.MapFS{}
	for p, text := range files {
		m[p] = &fstest.MapFile{Data: []byte(text)}
	}
	return lintRoom{name: name, owner: owner, foreign: foreign, sources: []lintSource{{prefix: "scaffold/" + name, fsys: m}}}
}

// AC1: a build persona that mentions "functional spec" fails, naming the file,
// line and term.
func TestLintBuildDocUsingAPlanTerm(t *testing.T) {
	got := lintDocs([]lintRoom{room(BuildRoom, PlanRoom, planOnlyTerms, map[string]string{
		"01_design/personas/analyst.md": "# analyst\n\nRead the Functional spec first.\n",
	})})
	want := []string{`scaffold/build/01_design/personas/analyst.md:3: plan-only term "functional spec" in the build room`}
	if strings.Join(got, "\n") != strings.Join(want, "\n") {
		t.Fatalf("problems:\n%s\nwant:\n%s", strings.Join(got, "\n"), strings.Join(want, "\n"))
	}
}

// AC6: a plan doc that uses a build-only term fails; shared terms pass in both.
func TestLintPlanDocUsingABuildTerm(t *testing.T) {
	shared := "Each ticket has requirements, a design, a review and one repo.\n"
	got := lintDocs([]lintRoom{
		room(PlanRoom, BuildRoom, buildOnlyTerms, map[string]string{
			"05_breakdown/context.md": shared + "Write tasks.json next.\n",
			"notes.txt":               "tasks.json in a non-markdown file is ignored\n",
		}),
		room(BuildRoom, PlanRoom, planOnlyTerms, map[string]string{"context.md": shared}),
	})
	want := `scaffold/plan/05_breakdown/context.md:2: build-only term "tasks.json" in the plan room`
	if len(got) != 1 || got[0] != want {
		t.Fatalf("problems = %q, want [%q]", got, want)
	}
}

// R3: any case, whole terms only.
func TestLintTermBoundaries(t *testing.T) {
	for line, want := range map[string]bool{
		"See ADR-0001.":                true,
		"FUNCTIONAL SPEC":              true,
		"(functional spec)":            true,
		"the adrDir setting":           false,
		"an encoder":                   false,
		"coders":                       false,
		"the coder writes it":          true,
		"plan/04_architecture/adr/x":   true,
		"functional specification":     false,
		"run `daiku pr` from the repo": true,
	} {
		term := "ADR"
		switch {
		case strings.Contains(strings.ToLower(line), "functional"):
			term = "functional spec"
		case strings.Contains(line, "cod"):
			term = "coder"
		case strings.Contains(line, "04_"):
			term = "04_architecture"
		case strings.Contains(line, "pr"):
			term = "daiku pr"
		}
		if got := termPattern(term).MatchString(line); got != want {
			t.Errorf("%q against %q = %v, want %v", term, line, got, want)
		}
	}
}

// R2: the shipped scaffold is clean, so `daiku check` passes.
func TestShippedScaffoldIsClean(t *testing.T) {
	if problems := Check(); len(problems) > 0 {
		t.Fatalf("daiku check problems:\n%s", strings.Join(problems, "\n"))
	}
}

// Every listed term is real vocabulary: it occurs in its own room's shipped
// docs, so the lists don't go stale as the docs change.
func TestLintTermsAreUsed(t *testing.T) {
	text := map[string]string{}
	for _, r := range scaffoldRooms() {
		var b strings.Builder
		for _, src := range r.sources {
			if src.file != "" {
				data, err := fs.ReadFile(src.fsys, src.file)
				if err != nil {
					t.Fatal(err)
				}
				b.Write(data)
				continue
			}
			fs.WalkDir(src.fsys, ".", func(p string, d fs.DirEntry, err error) error {
				if err == nil && !d.IsDir() && strings.HasSuffix(p, ".md") {
					data, _ := fs.ReadFile(src.fsys, p)
					b.Write(data)
				}
				return nil
			})
		}
		text[r.name] = b.String()
	}
	for owner, terms := range map[string][]string{PlanRoom: planOnlyTerms, BuildRoom: buildOnlyTerms} {
		for _, term := range terms {
			if !termPattern(term).MatchString(text[owner]) {
				t.Errorf("%s-only term %q isn't used anywhere in the %s room", owner, term, owner)
			}
		}
	}
}
