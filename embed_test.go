package daiku

import (
	"io/fs"
	"regexp"
	"strings"
	"testing"
)

func TestTreesExist(t *testing.T) {
	for name, fsys := range map[string]fs.FS{"root": RootFS, "plan": PlanFS} {
		entries, err := fs.ReadDir(fsys, ".")
		if err != nil || len(entries) == 0 {
			t.Errorf("%s tree is empty: %v", name, err)
		}
	}
}

// The root tree is root-only: no room folders and no AGENTS.md, which is
// assembled from fragments instead.
func TestRootTreeHasNoRoomsOrAgents(t *testing.T) {
	for _, p := range []string{"plan", "build", "specs", "00_intake", "AGENTS.md"} {
		if _, err := fs.Stat(RootFS, p); err == nil {
			t.Errorf("root tree has %s", p)
		}
	}
	for _, p := range []string{"CLAUDE.md", "README.md", ".gitignore", "ideas", "references"} {
		if _, err := fs.Stat(RootFS, p); err != nil {
			t.Errorf("root tree is missing %s", p)
		}
	}
}

// bareRoomPath finds a room path (specs/, 00_intake/ … 06_publish/) that isn't
// under plan/.
var bareRoomPath = regexp.MustCompile(`(^|[^\w./-])(specs/|0[0-6]_[a-z]+/)`)

// Every room path the scaffold's text names is under plan/, and no file names
// zumen or tomo as a command.
func TestScaffoldTextUsesPlanPaths(t *testing.T) {
	err := fs.WalkDir(ScaffoldFS, "scaffold", func(p string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return err
		}
		data, err := fs.ReadFile(ScaffoldFS, p)
		if err != nil {
			return err
		}
		for i, line := range strings.Split(string(data), "\n") {
			// A directory-tree drawing nests folders under plan/ itself.
			tree := strings.Contains(line, "├── ") || strings.Contains(line, "└── ")
			if m := bareRoomPath.FindStringSubmatch(line); m != nil && !tree {
				t.Errorf("%s:%d names %s outside plan/: %s", p, i+1, m[2], line)
			}
			for _, cmd := range []string{"`zumen ", "`tomo "} {
				if strings.Contains(line, cmd) {
					t.Errorf("%s:%d names %s: %s", p, i+1, strings.Trim(cmd, "` "), line)
				}
			}
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
}
