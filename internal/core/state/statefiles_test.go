package state

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/core/workspace"
)

// Security finding 1 (S-0040): every file the store keeps at a fixed path refuses
// to be written through a symlink, and the target is left as it was. A refused
// transition also changes nothing else: STATE.json and LOG.md stay as they were,
// so a gate is never recorded without being logged (S-0040 test rerun).
func TestStateFilesRefuseSymlinks(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	for _, file := range []string{"STATE.json", "LOG.md", ".daiku/active"} {
		t.Run(file, func(t *testing.T) {
			root, s := t.TempDir(), daikuStore()
			if _, err := s.Seed(root, "1-gateway", PlanLoopName, SeedOptions{}); err != nil {
				t.Fatal(err)
			}
			if _, err := s.Advance(root, "intake", Approved, ""); err != nil {
				t.Fatal(err)
			}
			link := filepath.Join(root, filepath.FromSlash(file))
			// The target holds what the file held, so the store can still read it
			// and only the write is what's refused.
			content, err := os.ReadFile(link)
			if err != nil {
				t.Fatal(err)
			}
			victim := filepath.Join(t.TempDir(), "victim")
			if err := os.WriteFile(victim, content, 0o644); err != nil {
				t.Fatal(err)
			}
			if err := os.Remove(link); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(victim, link); err != nil {
				t.Fatal(err)
			}
			stateBefore := read(t, s.StatePath(root))
			logBefore := read(t, s.LogPath(root))

			for name, run := range map[string]func() error{
				"Advance":     func() error { _, err := s.Advance(root, "publish", InProgress, ""); return err },
				"Reopen":      func() error { _, err := s.Reopen(root, "intake", "redo"); return err },
				"CloseChange": func() error { _, err := s.CloseChange(root, "1-gateway", "done"); return err },
				"Seed":        func() error { _, err := s.Seed(root, "2-other", PlanLoopName, SeedOptions{}); return err },
			} {
				if err := run(); err == nil || !strings.Contains(err.Error(), file+" is a symlink") {
					t.Fatalf("%s err = %v", name, err)
				}
				if got := read(t, s.StatePath(root)); got != stateBefore {
					t.Fatalf("%s changed STATE.json:\n%s", name, got)
				}
				if got := read(t, s.LogPath(root)); got != logBefore {
					t.Fatalf("%s changed LOG.md:\n%s", name, got)
				}
			}
			if got, _ := os.ReadFile(victim); string(got) != string(content) {
				t.Fatalf("the target was written through the symlink: %q", got)
			}
			if found := folderNamed(t, root, "2-other"); found != "" {
				t.Fatalf("a refused seed left %s behind", found)
			}
		})
	}
	t.Run("switch pointer", func(t *testing.T) {
		root, s := t.TempDir(), daikuStore()
		if _, err := s.Seed(root, "1-gateway", PlanLoopName, SeedOptions{}); err != nil {
			t.Fatal(err)
		}
		victim := filepath.Join(t.TempDir(), "authorized_keys")
		os.WriteFile(victim, []byte("ssh-ed25519 AAAA\n"), 0o600)
		active := filepath.Join(root, ".daiku", "active")
		os.Remove(active)
		os.Symlink(victim, active)
		if _, err := s.Switch(root, "1-gateway"); err == nil || !strings.Contains(err.Error(), "is a symlink") {
			t.Fatalf("Switch err = %v", err)
		}
		if got, _ := os.ReadFile(victim); string(got) != "ssh-ed25519 AAAA\n" {
			t.Fatalf("victim = %q", got)
		}
		if err := workspace.WriteActive(root, "daiku", "x"); err == nil {
			t.Fatal("WriteActive wrote through the symlink")
		}
	})
}

func read(t *testing.T, p string) string {
	t.Helper()
	b, err := os.ReadFile(p)
	if err != nil && !os.IsNotExist(err) {
		t.Fatal(err)
	}
	return string(b)
}

// folderNamed returns the path of a folder called name anywhere under root, or "".
func folderNamed(t *testing.T, root, name string) string {
	t.Helper()
	found := ""
	filepath.WalkDir(root, func(p string, d os.DirEntry, err error) error {
		if err == nil && d.IsDir() && d.Name() == name {
			found = p
		}
		return nil
	})
	return found
}
