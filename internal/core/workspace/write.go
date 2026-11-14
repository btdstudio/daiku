package workspace

import (
	"fmt"
	"os"
	"path/filepath"
)

// CheckPath refuses a path that isn't safe to write under root: one that lies
// outside root, or whose path from root passes through a symlink, including a
// symlink in the final file's place. A committed symlink could otherwise send a
// write anywhere the person can write. path is the file's full path, joined to
// root; a path that doesn't exist yet is fine.
func CheckPath(root, path string) error {
	rel, err := filepath.Rel(root, path)
	if err != nil || !filepath.IsLocal(rel) {
		return fmt.Errorf("daiku: won't write %s: it's outside the workspace", path)
	}
	if link := ThroughSymlink(root, filepath.ToSlash(rel)); link != "" {
		return fmt.Errorf("daiku: won't write %s: %s is a symlink, which could lead outside the workspace", filepath.ToSlash(rel), link)
	}
	return nil
}

// MkdirAll is os.MkdirAll for a folder under root, after CheckPath.
func MkdirAll(root, path string, perm os.FileMode) error {
	if err := CheckPath(root, path); err != nil {
		return err
	}
	return os.MkdirAll(path, perm)
}

// WriteFile is os.WriteFile for a file under root, after CheckPath. It creates
// the file's folder when it's missing.
func WriteFile(root, path string, data []byte, perm os.FileMode) error {
	if err := MkdirAll(root, filepath.Dir(path), 0o755); err != nil {
		return err
	}
	if err := CheckPath(root, path); err != nil {
		return err
	}
	return os.WriteFile(path, data, perm)
}

// OpenFile is os.OpenFile for a file under root, after CheckPath. It doesn't
// create folders.
func OpenFile(root, path string, flag int, perm os.FileMode) (*os.File, error) {
	if err := CheckPath(root, path); err != nil {
		return nil, err
	}
	return os.OpenFile(path, flag, perm)
}
