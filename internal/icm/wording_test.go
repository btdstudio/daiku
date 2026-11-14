package icm

import (
	"io/fs"
	"regexp"
	"strings"
	"testing"

	daiku "github.com/btdstudio/daiku"
)

var (
	commitWord = regexp.MustCompile(`(?i)\bcommit`)
	listStart  = regexp.MustCompile(`^(\s*[-*]|\s*\d+\.)\s`)
)

// mdBlocks splits markdown into the units a reader takes as one statement: a list
// item with its indented continuation lines, a paragraph, a heading, or a single
// line inside a code fence. Each block carries the heading it sits under.
func mdBlocks(text string) (blocks, headings []string) {
	var cur []string
	heading := ""
	inFence := false
	flush := func() {
		if len(cur) > 0 {
			blocks = append(blocks, strings.Join(cur, " "))
			headings = append(headings, heading)
			cur = nil
		}
	}
	for _, line := range strings.Split(text, "\n") {
		trimmed := strings.TrimSpace(line)
		switch {
		case strings.HasPrefix(trimmed, "```"):
			flush()
			inFence = !inFence
		case inFence:
			cur = []string{line}
			flush()
		case trimmed == "":
			flush()
		case strings.HasPrefix(trimmed, "#"):
			flush()
			heading = trimmed
		case listStart.MatchString(line):
			flush()
			cur = []string{line}
		default:
			cur = append(cur, line)
		}
	}
	flush()
	return blocks, headings
}

// AC12: every scaffolded statement about committing is qualified by vcs: git, so
// a local workspace's instructions never promise commits.
func TestScaffoldCommitWordingIsConditional(t *testing.T) {
	for rel, data := range scaffoldMarkdown(t) {
		blocks, headings := mdBlocks(data)
		for i, block := range blocks {
			if !commitWord.MatchString(block) {
				continue
			}
			if headings[i] == "## Version control" || strings.Contains(block, "vcs: git") {
				continue
			}
			t.Errorf("%s: commit wording not qualified by `vcs: git`: %s", rel, strings.TrimSpace(block))
		}
	}
}

// N6: each room's text names only its own loop's phases. The build room never
// calls a phase intake or plan, and the plan room never calls one ticket or tasks.
func TestRoomsNameOnlyTheirOwnPhases(t *testing.T) {
	buildBans := regexp.MustCompile(`\bintake\b|\*\*plan\*\*|\bplan phase|plan →|→ plan\b`)
	planBans := regexp.MustCompile(`\*\*ticket\*\*|\*\*tasks\*\*|ticket →|→ tasks\b|\btasks phase`)
	check := func(name, text string, bans *regexp.Regexp) {
		if m := bans.FindString(text); m != "" {
			t.Errorf("%s names another loop's phase: %q", name, m)
		}
	}
	for name, fsys := range map[string]fs.FS{"build/": daiku.BuildFS, "plan/": daiku.PlanFS} {
		bans := planBans
		if name == "build/" {
			bans = buildBans
		}
		err := fs.WalkDir(fsys, ".", func(rel string, d fs.DirEntry, err error) error {
			if err != nil || d.IsDir() {
				return err
			}
			data, err := fs.ReadFile(fsys, rel)
			check(name+rel, string(data), bans)
			return err
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	for room, bans := range map[string]*regexp.Regexp{"build.md": buildBans, "plan.md": planBans} {
		data, err := fs.ReadFile(daiku.AgentsFS, room)
		if err != nil {
			t.Fatal(err)
		}
		check("agents/"+room, string(data), bans)
	}
}
