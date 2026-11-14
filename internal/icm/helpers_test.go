package icm

import (
	"crypto/sha256"
	"encoding/hex"
	"io/fs"
	"regexp"
	"testing"

	"github.com/btdstudio/daiku"
	"github.com/btdstudio/daiku/internal/agents"
)

func hashBytes(data []byte) string {
	sum := sha256.Sum256(data)
	return hex.EncodeToString(sum[:])
}

var roomPath = regexp.MustCompile(`^(0[0-6]_[a-z]+|specs)(/|$)`)

// scaffoldFS finds a scaffold file by the path zumen's tests used: AGENTS.md is
// the assembled file with the plan room, a room folder is in the plan tree, and
// anything else is in the root tree.
func scaffoldText(t *testing.T, rel string) string {
	t.Helper()
	if rel == "AGENTS.md" {
		text, err := agents.Assemble("plan")
		if err != nil {
			t.Fatal(err)
		}
		return text
	}
	fsys := daiku.RootFS
	if roomPath.MatchString(rel) {
		fsys = daiku.PlanFS
	}
	data, err := fs.ReadFile(fsys, rel)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

// scaffoldMarkdown is every .md the scaffold ships, by workspace path, plus the
// assembled AGENTS.md.
func scaffoldMarkdown(t *testing.T) map[string]string {
	t.Helper()
	files := map[string]string{"AGENTS.md": scaffoldText(t, "AGENTS.md")}
	for prefix, fsys := range map[string]fs.FS{"": daiku.RootFS, "plan/": daiku.PlanFS} {
		err := fs.WalkDir(fsys, ".", func(rel string, d fs.DirEntry, err error) error {
			if err != nil || d.IsDir() || len(rel) < 3 || rel[len(rel)-3:] != ".md" {
				return err
			}
			data, err := fs.ReadFile(fsys, rel)
			files[prefix+rel] = string(data)
			return err
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	return files
}
