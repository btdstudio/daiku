package workspace

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestPaths(t *testing.T) {
	if got, want := Dir("root", "zumen"), filepath.Join("root", ".zumen"); got != want {
		t.Errorf("Dir = %q, want %q", got, want)
	}
	if got, want := ActivePath("root", "tomo"), filepath.Join("root", ".tomo", "active"); got != want {
		t.Errorf("ActivePath = %q, want %q", got, want)
	}
}

func TestActiveRoundTrip(t *testing.T) {
	root := t.TempDir()
	got, err := ReadActive(root, "zumen")
	if err != nil || got != "" {
		t.Fatalf("ReadActive before write = %q, %v", got, err)
	}
	if err := WriteActive(root, "zumen", "1-sample"); err != nil {
		t.Fatal(err)
	}
	got, err = ReadActive(root, "zumen")
	if err != nil || got != "1-sample" {
		t.Fatalf("ReadActive after write = %q, %v", got, err)
	}
}

func TestCheckLegacy(t *testing.T) {
	for _, tool := range []string{"zumen", "tomo"} {
		t.Run(tool, func(t *testing.T) {
			root := t.TempDir()
			if err := os.Mkdir(filepath.Join(root, "."+tool), 0o755); err != nil {
				t.Fatal(err)
			}
			err := CheckLegacy(root)
			if err == nil || !strings.Contains(err.Error(), "a "+tool+" workspace") {
				t.Fatalf("CheckLegacy = %v, want an error naming %s", err, tool)
			}
		})
	}
	t.Run("both name zumen first", func(t *testing.T) {
		root := t.TempDir()
		for _, d := range []string{".zumen", ".tomo"} {
			if err := os.Mkdir(filepath.Join(root, d), 0o755); err != nil {
				t.Fatal(err)
			}
		}
		if err := CheckLegacy(root); err == nil || !strings.Contains(err.Error(), "zumen") {
			t.Fatalf("CheckLegacy = %v, want zumen named", err)
		}
	})
	t.Run("a file is not a workspace", func(t *testing.T) {
		root := t.TempDir()
		if err := os.WriteFile(filepath.Join(root, ".zumen"), nil, 0o644); err != nil {
			t.Fatal(err)
		}
		if err := CheckLegacy(root); err != nil {
			t.Fatalf("CheckLegacy = %v, want nil", err)
		}
	})
	t.Run("empty dir", func(t *testing.T) {
		if err := CheckLegacy(t.TempDir()); err != nil {
			t.Fatalf("CheckLegacy = %v, want nil", err)
		}
	})
}

func TestThroughSymlink(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "a", "b"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(filepath.Join(root, "a"), filepath.Join(root, "l")); err != nil {
		t.Fatal(err)
	}
	cases := map[string]string{
		"a/b":       "",
		"a/b/c/d":   "", // missing components are fine
		"missing/x": "",
		"l/b":       filepath.Join(root, "l"),
		"l":         filepath.Join(root, "l"),
	}
	for rel, want := range cases {
		if got := ThroughSymlink(root, rel); got != want {
			t.Errorf("ThroughSymlink(%q) = %q, want %q", rel, got, want)
		}
	}
}

func TestCheckPath(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "a"), 0o755); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	if runtime.GOOS != "windows" {
		if err := os.Symlink(outside, filepath.Join(root, "linkdir")); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(filepath.Join(outside, "victim"), filepath.Join(root, "linkfile")); err != nil {
			t.Fatal(err)
		}
	}
	ok := []string{"a/x.md", "STATE.json", "new/deeper/x.md"}
	for _, rel := range ok {
		if err := CheckPath(root, filepath.Join(root, filepath.FromSlash(rel))); err != nil {
			t.Errorf("CheckPath(%s) = %v", rel, err)
		}
	}
	bad := map[string]string{
		"../escape/x.md": "outside the workspace",
		"a/../../x":      "outside the workspace",
	}
	if runtime.GOOS != "windows" {
		bad["linkdir/x.md"] = "linkdir is a symlink"
		bad["linkfile"] = "linkfile is a symlink"
	}
	for rel, want := range bad {
		err := CheckPath(root, filepath.Join(root, filepath.FromSlash(rel)))
		if err == nil || !strings.Contains(err.Error(), want) {
			t.Errorf("CheckPath(%s) = %v, want %q", rel, err, want)
		}
	}
	if err := CheckPath(root, filepath.Join(t.TempDir(), "elsewhere")); err == nil {
		t.Error("CheckPath accepted a path in another folder")
	}
}

func TestWriteHelpers(t *testing.T) {
	root := t.TempDir()
	p := filepath.Join(root, "x", "y.md")
	if err := WriteFile(root, p, []byte("hi"), 0o644); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(p); string(got) != "hi" {
		t.Fatalf("file = %q", got)
	}
	f, err := OpenFile(root, p, os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		t.Fatal(err)
	}
	f.WriteString("!")
	f.Close()
	if got, _ := os.ReadFile(p); string(got) != "hi!" {
		t.Fatalf("file = %q", got)
	}
	if runtime.GOOS == "windows" {
		return
	}
	outside := t.TempDir()
	victim := filepath.Join(outside, "victim")
	os.WriteFile(victim, []byte("precious"), 0o644)
	if err := os.Symlink(victim, filepath.Join(root, "link")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "dir")); err != nil {
		t.Fatal(err)
	}
	for name, run := range map[string]func() error{
		"WriteFile through a file link":   func() error { return WriteFile(root, filepath.Join(root, "link"), []byte("x"), 0o644) },
		"WriteFile through a folder link": func() error { return WriteFile(root, filepath.Join(root, "dir", "n"), []byte("x"), 0o644) },
		"MkdirAll through a folder link":  func() error { return MkdirAll(root, filepath.Join(root, "dir", "sub"), 0o755) },
		"OpenFile append through a link": func() error {
			f, err := OpenFile(root, filepath.Join(root, "link"), os.O_APPEND|os.O_WRONLY, 0o644)
			if f != nil {
				f.Close()
			}
			return err
		},
	} {
		if err := run(); err == nil || !strings.Contains(err.Error(), "is a symlink") {
			t.Errorf("%s: err = %v", name, err)
		}
	}
	if got, _ := os.ReadFile(victim); string(got) != "precious" {
		t.Errorf("victim changed: %q", got)
	}
	if entries, _ := os.ReadDir(outside); len(entries) != 1 {
		t.Errorf("outside has %d entries", len(entries))
	}
}
