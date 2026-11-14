package state

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/workspace"
)

// Security finding 1: a symlinked folder on the way to a change folder (such as
// a committed build/<repo> or plan/specs) must not let daiku write outside the
// workspace.
func TestSymlinkedParentsAreRefused(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	cases := []struct {
		name, loop, change, link string
		repos                    []string
	}{
		{"build repo folder", BuildLoopName, "S-0036", "build/api", []string{"api"}},
		{"build room", BuildLoopName, "S-0036", "build", []string{"api"}},
		{"build changes folder", BuildLoopName, "S-0036", "build/api/changes", []string{"api"}},
		{"plan specs", PlanLoopName, "1-gateway", "plan/specs", nil},
		{"plan room", PlanLoopName, "1-gateway", "plan", nil},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			base := t.TempDir()
			root, outside := filepath.Join(base, "ws"), filepath.Join(base, "outside")
			link := filepath.Join(root, filepath.FromSlash(tc.link))
			for _, d := range []string{filepath.Dir(link), outside} {
				if err := os.MkdirAll(d, 0o755); err != nil {
					t.Fatal(err)
				}
			}
			if err := os.Symlink(outside, link); err != nil {
				t.Fatal(err)
			}
			s := daikuStore()
			s.VCS = "git" // so Seed would write a .gitkeep; Commit stays off
			_, err := s.Seed(root, tc.change, tc.loop, SeedOptions{Repos: tc.repos})
			if err == nil || !strings.Contains(err.Error(), "is a symlink") {
				t.Fatalf("Seed err = %v", err)
			}
			if entries, _ := os.ReadDir(outside); len(entries) != 0 {
				t.Fatalf("Seed wrote %d entries outside the workspace", len(entries))
			}
			if _, err := os.Stat(s.StatePath(root)); !os.IsNotExist(err) {
				t.Fatal("a refused Seed wrote STATE.json")
			}
		})
	}
}

// A change whose folder was reached through a symlink planted after it was
// seeded can't be switched to, gated, reopened or closed.
func TestSymlinkPlantedAfterSeedIsRefused(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	base := t.TempDir()
	root, outside := filepath.Join(base, "ws"), filepath.Join(base, "outside")
	s := daikuStore()
	if _, err := s.Seed(root, "S-0036", BuildLoopName, SeedOptions{Repos: []string{"api"}}); err != nil {
		t.Fatal(err)
	}
	// Swap build/api for a symlink to a look-alike tree outside.
	if err := os.MkdirAll(filepath.Join(outside, "changes", "S-0036"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.RemoveAll(filepath.Join(root, "build", "api")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "build", "api")); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Switch(root, "S-0036"); err == nil || !strings.Contains(err.Error(), "unknown change") {
		t.Errorf("Switch err = %v", err)
	}
	if err := workspace.WriteActive(root, "daiku", "S-0036"); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Advance(root, "ticket", Approved, ""); err == nil || !strings.Contains(err.Error(), "isn't a valid change folder") {
		t.Errorf("Advance err = %v", err)
	}
	if _, err := s.Reopen(root, "ticket", "r"); err == nil || !strings.Contains(err.Error(), "isn't a valid change folder") {
		t.Errorf("Reopen err = %v", err)
	}
	if _, err := s.CloseChange(root, "S-0036", "r"); err == nil {
		t.Error("CloseChange accepted a symlinked folder")
	}
	if _, err := s.ChangeDir(root, Change{Name: "S-0036", Loop: BuildLoopName, Repos: []string{"api"}}); err != nil {
		t.Errorf("ChangeDir is a pure path helper and shouldn't fail: %v", err)
	}
}
