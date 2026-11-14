package icm

import (
	"regexp"
	"strings"
	"testing"
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
