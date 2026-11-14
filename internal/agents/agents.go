// Package agents builds a workspace's AGENTS.md. The file is assembled from a
// root fragment, which every workspace has, plus one fragment per room the
// workspace has, and merged into the person's file by `##` section, so their own
// sections and notes survive (ADR-0007).
package agents

import (
	"errors"
	"fmt"
	"io/fs"
	"strings"

	"github.com/btdstudio/daiku"
)

// Rooms are the rooms with an AGENTS.md fragment, in the order their sections
// follow the root's.
var Rooms = []string{"plan", "build"}

// Assemble returns the root fragment followed by each given room's fragment, in
// Rooms order whatever order they're passed in. A room daiku doesn't have a
// fragment for yet (build, until its version) contributes nothing; an unknown
// room is an error.
func Assemble(rooms ...string) (string, error) {
	root, err := fs.ReadFile(daiku.AgentsFS, "root.md")
	if err != nil {
		return "", err
	}
	want := map[string]bool{}
	for _, r := range rooms {
		known := false
		for _, k := range Rooms {
			known = known || r == k
		}
		if !known {
			return "", fmt.Errorf("daiku: no AGENTS.md fragment for room %q", r)
		}
		want[r] = true
	}
	parts := []string{strings.TrimRight(string(root), "\n")}
	for _, r := range Rooms {
		if !want[r] {
			continue
		}
		data, err := fs.ReadFile(daiku.AgentsFS, r+".md")
		if err != nil {
			if errors.Is(err, fs.ErrNotExist) {
				continue
			}
			return "", err
		}
		parts = append(parts, strings.TrimRight(string(data), "\n"))
	}
	return strings.Join(parts, "\n\n") + "\n", nil
}
