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
