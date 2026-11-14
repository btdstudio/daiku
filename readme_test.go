package daiku_test

import (
	"os"
	"slices"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/cli"
)

func readme(t *testing.T) string {
	t.Helper()
	data, err := os.ReadFile("README.md")
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

// The README's command table lists every command, in usage order, so it can't
// drift from `daiku help`.
func TestReadmeCommandTableMatchesUsage(t *testing.T) {
	var rows []string
	for _, line := range strings.Split(readme(t), "\n") {
		if !strings.HasPrefix(line, "| `daiku ") {
			continue
		}
		cell := strings.TrimPrefix(line, "| `")
		end := strings.Index(cell, "` |")
		if end < 0 {
			t.Fatalf("malformed command row: %q", line)
		}
		rows = append(rows, strings.ReplaceAll(cell[:end], `\|`, "|"))
	}
	if want := cli.Synopses(); !slices.Equal(rows, want) {
		t.Errorf("README command table:\n%s\nwant (daiku help):\n%s", strings.Join(rows, "\n"), strings.Join(want, "\n"))
	}
}

// The README describes breakdown, ideas and references as the plan room's
// scaffold does (carried over from zumen's README checks).
func TestReadmeWording(t *testing.T) {
	flat := strings.Join(strings.Fields(readme(t)), " ")
	for _, want := range []string{
		"features and epics only where grouping helps",
		"references/ reference files + your own index.md (daiku never edits it)",
		"`daiku references check`",
		"ideas/ ideas, one <n>-<slug>/ folder each (summary.md, log.md)",
		"## Ideas",
		"`daiku idea promote <id> --title <t>`",
		"parked or dropped, never deleted",
		"`daiku idea new`", "`daiku change new`", "`daiku build new`",
	} {
		if !strings.Contains(flat, want) {
			t.Errorf("README.md lacks %q", want)
		}
	}
	for _, old := range []string{"epics, features, stories and tasks", "epics, then features"} {
		if strings.Contains(flat, old) {
			t.Errorf("README.md still says %q", old)
		}
	}
}
