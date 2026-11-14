package cli

import (
	"os"
	"strings"
	"testing"
)

// QA for S-0043 D6: only a reference that resolves to nothing is relaxed. An
// inbox ticket whose parent is here but of the wrong type still fails, and
// one whose depends_on is another inbox ticket raises nothing.
func TestQAInboxReferenceRules(t *testing.T) {
	root := rootOnly(t)
	if code := repoCmd(root, "add", "api", "https://github.com/org/api"); code != 0 {
		t.Fatal("repo add")
	}
	pull := func(pm, extra string) {
		t.Helper()
		draft := writeDraft(t, "github: \""+pm+"\"", "Item "+pm)
		text := strings.Replace(readFile(t, draft), "status: draft\n", "status: draft\n"+extra, 1)
		if err := os.WriteFile(draft, []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
		if code := runTickets(root, []string{"pull", draft}); code != 0 {
			t.Fatalf("pull %s", pm)
		}
	}
	pull("#1", "")
	pull("#2", "depends_on: [X-1]\n")
	code, out := captureStdout(t, func() int { return runTickets(root, []string{"check"}) })
	if code != 0 || strings.Contains(out, "!") || strings.Contains(out, "✗") {
		t.Fatalf("inbox depends_on within the inbox = %d:\n%s", code, out)
	}
	pull("#3", "parent: X-1\n") // a task can't be a task's parent
	code, out = captureStdout(t, func() int { return runTickets(root, []string{"check"}) })
	if code != 1 || !strings.Contains(out, "✗ X-3: parent X-1 is a task") {
		t.Fatalf("a present parent of the wrong type = %d:\n%s", code, out)
	}
}
