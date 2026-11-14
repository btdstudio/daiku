package agents

import (
	"strings"
	"testing"
)

func headings(t *testing.T, text string) []string {
	t.Helper()
	var out []string
	for _, sec := range parseAgents(text).sections {
		out = append(out, sec.heading)
	}
	return out
}

// AC2: a root-only workspace's AGENTS.md names idea new, change new and build new,
// and says what each is for, with no plan sections yet.
func TestRootOnlyNamesEveryWayIn(t *testing.T) {
	text, err := Assemble()
	if err != nil {
		t.Fatal(err)
	}
	doc := parseAgents(text)
	var start string
	for _, sec := range doc.sections {
		if sec.heading == "How to start" {
			start = strings.Join(sec.body, "\n")
		}
	}
	for cmd, purpose := range map[string]string{
		"daiku idea new":   "Think an idea through",
		"daiku change new": "Plan an initiative",
		"daiku build new":  "Build a ticket",
	} {
		if !strings.Contains(start, cmd) || !strings.Contains(start, purpose) {
			t.Errorf("How to start doesn't name %s with %q:\n%s", cmd, purpose, start)
		}
	}
	for _, h := range headings(t, text) {
		if strings.HasPrefix(h, "Plan ") {
			t.Errorf("root-only AGENTS.md has %q", h)
		}
	}
}

// R4: with the plan room, every Plan section follows the root's, and no heading
// repeats across fragments.
func TestAssembleWithPlan(t *testing.T) {
	root, err := Assemble()
	if err != nil {
		t.Fatal(err)
	}
	full, err := Assemble("plan")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(full, strings.TrimRight(root, "\n")) {
		t.Error("the assembled file doesn't start with the root fragment")
	}
	seen := map[string]bool{}
	var plan []string
	for _, h := range headings(t, full) {
		if seen[h] {
			t.Errorf("heading %q repeats", h)
		}
		seen[h] = true
		if strings.HasPrefix(h, "Plan ") {
			plan = append(plan, h)
		}
	}
	want := []string{"Plan loop", "Plan commands", "Plan routing", "Plan naming", "Plan directory map", "Plan handoff"}
	if strings.Join(plan, ",") != strings.Join(want, ",") {
		t.Errorf("plan sections = %v, want %v", plan, want)
	}
	// Order doesn't depend on how rooms are passed: plan's sections, then build's,
	// and no heading repeats across the three fragments.
	both, err := Assemble("plan", "build")
	if err != nil {
		t.Fatal(err)
	}
	again, err := Assemble("build", "plan")
	if err != nil || again != both {
		t.Errorf("Assemble(build, plan) differs from Assemble(plan, build): %v", err)
	}
	if !strings.HasPrefix(both, strings.TrimRight(full, "\n")) {
		t.Error("the build sections don't follow the plan sections")
	}
	seen = map[string]bool{}
	var build []string
	for _, h := range headings(t, both) {
		if seen[h] {
			t.Errorf("heading %q repeats", h)
		}
		seen[h] = true
		if strings.HasPrefix(h, "Build ") {
			build = append(build, h)
		}
	}
	if want := []string{"Build loop", "Build routing", "Build naming", "Build directory map"}; strings.Join(build, ",") != strings.Join(want, ",") {
		t.Errorf("build sections = %v, want %v", build, want)
	}
	if _, err := Assemble("ideas"); err == nil {
		t.Error("an unknown room was accepted")
	}
}

// AC4: merging the assembled file into a person's AGENTS.md keeps their own
// section and refreshes daiku's, and adding the plan room only adds sections.
func TestMergeKeepsTheirSectionAndAddsTheRoom(t *testing.T) {
	root, _ := Assemble()
	user := strings.Replace(root, "## Version control\n", "## Version control\n\nstale text\n", 1) + "\n## Team notes\n\nkeep me\n"
	full, _ := Assemble("plan")
	got, reason := Merge(user, full)
	if reason != "" {
		t.Fatal(reason)
	}
	if !strings.Contains(got, "## Team notes\n\nkeep me\n") {
		t.Error("their section was lost")
	}
	if strings.Contains(got, "stale text") {
		t.Error("daiku's Version control wasn't refreshed")
	}
	for _, h := range []string{"## Plan loop", "## Plan handoff"} {
		if !strings.Contains(got, h) {
			t.Errorf("merge didn't add %s", h)
		}
	}
	if again, _ := Merge(got, full); again != got {
		t.Error("merge isn't idempotent")
	}
}
