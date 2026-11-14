// Package workspace owns a tool's local .<tool>/ directory and its active-change
// pointer.
package workspace

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
)

// DirName is the name of the tool's workspace directory (".<tool>").
func DirName(tool string) string { return "." + tool }

// Dir is the path to the tool's workspace directory under root.
func Dir(root, tool string) string { return filepath.Join(root, DirName(tool)) }

// ActivePath is the path to the file naming the active change.
func ActivePath(root, tool string) string { return filepath.Join(Dir(root, tool), "active") }

// ReadActive returns the active change's name ("" when none is set).
func ReadActive(root, tool string) (string, error) {
	p := ActivePath(root, tool)
	data, err := os.ReadFile(p)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", fmt.Errorf("%s: cannot read %s: %v", tool, p, err)
	}
	return strings.TrimSpace(string(data)), nil
}

// WriteActive records name as the active change.
func WriteActive(root, tool, name string) error {
	return WriteFile(root, ActivePath(root, tool), []byte(name+"\n"), 0o644)
}

// legacyTools are the tools daiku replaced. Their workspaces aren't migrated, so
// daiku refuses to act in one rather than writing a daiku state file over it.
var legacyTools = []string{"zumen", "tomo"}

// CheckLegacy fails when root is a zumen or tomo workspace, which is recognized
// by its .zumen/ or .tomo/ folder. The error names the tool.
func CheckLegacy(root string) error {
	for _, tool := range legacyTools {
		info, err := os.Stat(filepath.Join(root, DirName(tool)))
		if err == nil && info.IsDir() {
			return fmt.Errorf("daiku: this is a %s workspace (found %s/) — use %s, or start a new daiku workspace", tool, DirName(tool), tool)
		}
	}
	return nil
}

// ThroughSymlink returns the first existing path component of rel, a
// slash-separated path under root, that is a symlink, or "" if there's none.
// Components that don't exist yet are fine: they'll be created as real folders
// or files. Checking every component, not only the last, keeps a committed
// symlink such as build/<repo>, plan/specs or references/ from sending a write
// outside the workspace. root itself isn't checked, so a workspace may live
// under a symlinked path.
func ThroughSymlink(root, rel string) string {
	cur := root
	for _, part := range strings.Split(path.Clean(filepath.ToSlash(rel)), "/") {
		cur = filepath.Join(cur, part)
		info, err := os.Lstat(cur)
		if err != nil {
			return ""
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return cur
		}
	}
	return ""
}
