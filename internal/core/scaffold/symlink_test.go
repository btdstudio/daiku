package scaffold

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"testing/fstest"
)

// symlinkEngine has root files in a folder, a seed under .daiku/, and a room,
// so every kind of write the engine makes can meet a symlink.
func symlinkEngine() Engine {
	e := roomEngine()
	e.FS = fstest.MapFS{
		"AGENTS.md":            {Data: []byte("agents\n")},
		"references/README.md": {Data: []byte("refs\n")},
	}
	return e
}

func plantLink(t *testing.T, dir, rel string) string {
	t.Helper()
	outside := t.TempDir()
	link := filepath.Join(dir, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(link), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, link); err != nil {
		t.Fatal(err)
	}
	return outside
}

func emptyDir(t *testing.T, p string) {
	t.Helper()
	if entries, _ := os.ReadDir(p); len(entries) != 0 {
		t.Fatalf("%d entries were written outside the workspace", len(entries))
	}
}

// Every engine write refuses to go through a symlinked folder (S-0040 N3).
func TestWritesRefuseSymlinkedFolders(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	t.Run("init into a symlinked root folder", func(t *testing.T) {
		dir := t.TempDir()
		outside := plantLink(t, dir, "references")
		_, err := symlinkEngine().Scaffold(dir, Options{NoGit: true})
		if err == nil || !strings.Contains(err.Error(), "won't write references/README.md") {
			t.Fatalf("Scaffold err = %v", err)
		}
		emptyDir(t, outside)
	})
	t.Run("init seeds into a symlinked folder", func(t *testing.T) {
		dir := t.TempDir()
		outside := plantLink(t, dir, ".daiku")
		_, err := symlinkEngine().Scaffold(dir, Options{NoGit: true})
		if err == nil || !strings.Contains(err.Error(), ".daiku is a symlink") {
			t.Fatalf("Scaffold err = %v", err)
		}
		emptyDir(t, outside)
	})
	t.Run("update through a symlinked root folder", func(t *testing.T) {
		e := symlinkEngine()
		dir := rootOnly(t, e)
		if err := os.RemoveAll(filepath.Join(dir, "references")); err != nil {
			t.Fatal(err)
		}
		outside := plantLink(t, dir, "references")
		if _, err := e.Update(dir); err == nil || !strings.Contains(err.Error(), "references is a symlink") {
			t.Fatalf("Update err = %v", err)
		}
		emptyDir(t, outside)
	})
	t.Run("update through a symlinked folder inside a room", func(t *testing.T) {
		e := symlinkEngine()
		dir := rootOnly(t, e)
		if _, err := e.EnsureRoom(dir, "build"); err != nil {
			t.Fatal(err)
		}
		if err := os.RemoveAll(filepath.Join(dir, "build", "personas")); err != nil {
			t.Fatal(err)
		}
		outside := plantLink(t, dir, "build/personas")
		if _, err := e.Update(dir); err == nil || !strings.Contains(err.Error(), "personas is a symlink") {
			t.Fatalf("Update err = %v", err)
		}
		emptyDir(t, outside)
	})
}
