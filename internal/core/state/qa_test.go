package state

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/workspace"
)

// QA (AC4): every store entry point refuses a zumen or tomo workspace and
// leaves daiku's files exactly as they were.
func TestQALegacyRefusedOnEveryEntryPoint(t *testing.T) {
	for _, tool := range []string{"zumen", "tomo"} {
		t.Run(tool, func(t *testing.T) {
			root, s := t.TempDir(), daikuStore()
			if _, err := s.Seed(root, "1-gateway", PlanLoopName, SeedOptions{}); err != nil {
				t.Fatal(err)
			}
			before, err := os.ReadFile(s.StatePath(root))
			if err != nil {
				t.Fatal(err)
			}
			if err := os.Mkdir(filepath.Join(root, "."+tool), 0o755); err != nil {
				t.Fatal(err)
			}
			want := "a " + tool + " workspace"
			calls := map[string]func() error{
				"Load":        func() error { _, err := s.Load(root); return err },
				"ReadChanges": func() error { _, err := s.ReadChanges(root); return err },
				"Known":       func() error { _, err := s.Known(root); return err },
				"NextID":      func() error { _, err := s.NextID(root, PlanLoopName); return err },
				"Switch":      func() error { _, err := s.Switch(root, "1-gateway"); return err },
				"Seed":        func() error { _, err := s.Seed(root, "2-next", PlanLoopName, SeedOptions{}); return err },
				"Advance":     func() error { _, err := s.Advance(root, "intake", Approved, ""); return err },
				"Reopen":      func() error { _, err := s.Reopen(root, "intake", "r"); return err },
				"CloseChange": func() error { _, err := s.CloseChange(root, "1-gateway", "r"); return err },
			}
			for name, call := range calls {
				if err := call(); err == nil || !strings.Contains(err.Error(), want) {
					t.Errorf("%s: err = %v, want %q", name, err, want)
				}
			}
			after, _ := os.ReadFile(s.StatePath(root))
			if !bytes.Equal(before, after) {
				t.Error("STATE.json changed in a legacy workspace")
			}
			if _, err := os.Stat(filepath.Join(root, "plan", "specs", "2-next")); !os.IsNotExist(err) {
				t.Error("Seed created a folder in a legacy workspace")
			}
		})
	}
}

// QA (AC2, AC3): a hand-edited record can't steer a build change's folder out of
// the workspace or past the name rule.
func TestQACraftedBuildRecordsAreRefused(t *testing.T) {
	cases := map[string]string{
		"repo escapes":    `"X-1": {"loop": "build", "phase": "ticket", "status": "draft", "gates": {"ticket": "draft"}, "sources": [], "repos": ["../../evil"]}`,
		"no repo":         `"X-1": {"loop": "build", "phase": "ticket", "status": "draft", "gates": {"ticket": "draft"}, "sources": []}`,
		"plan-style name": `"7-gateway": {"loop": "build", "phase": "ticket", "status": "draft", "gates": {"ticket": "draft"}, "sources": [], "repos": ["api"]}`,
	}
	for name, record := range cases {
		t.Run(name, func(t *testing.T) {
			root, s := t.TempDir(), daikuStore()
			change := strings.Trim(strings.SplitN(record, ":", 2)[0], `"`)
			raw := `{"schema": 1, "changes": {` + record + `}}`
			if err := os.WriteFile(s.StatePath(root), []byte(raw), 0o644); err != nil {
				t.Fatal(err)
			}
			// Make every folder the record could point at, so only the rules refuse.
			for _, dir := range []string{
				filepath.Join(root, "build", "api", "changes", change),
				filepath.Join(filepath.Dir(root), "evil", "changes", change),
			} {
				if err := os.MkdirAll(dir, 0o755); err != nil {
					t.Fatal(err)
				}
			}
			t.Cleanup(func() { os.RemoveAll(filepath.Join(filepath.Dir(root), "evil")) })
			if _, err := s.Switch(root, change); err == nil {
				t.Error("Switch accepted the record")
			}
			if err := workspace.WriteActive(root, "daiku", change); err != nil {
				t.Fatal(err)
			}
			if _, err := s.Advance(root, "ticket", Approved, ""); err == nil {
				t.Error("Advance accepted the record")
			}
			if _, err := s.CloseChange(root, change, "r"); err == nil {
				t.Error("CloseChange accepted the record")
			}
			if _, err := s.ChangeDir(root, Change{Name: change, Loop: BuildLoopName, Repos: []string{"../../evil"}}); err == nil {
				t.Error("ChangeDir accepted an escaping repo")
			}
		})
	}
}
