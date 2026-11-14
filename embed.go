// Package daiku embeds the workspace scaffold so the daiku binary is
// self-contained: no source checkout is needed at runtime. `daiku init` copies
// the root tree out, a room's tree is copied the first time the room is used,
// and `daiku update` refreshes managed files against them.
//
// `all:` includes dot-files (.gitignore, .gitkeep), which a plain go:embed
// pattern would silently ignore.
package daiku

import (
	"embed"
	"io/fs"
)

// ScaffoldFS is the whole embedded scaffold: root/, plan/, build/ and agents/.
//
//go:embed all:scaffold
var ScaffoldFS embed.FS

// RootFS is the tree `daiku init` writes at the workspace root.
var RootFS = mustSub("scaffold/root")

// PlanFS is the plan room's tree, written under plan/ the first time it's used.
var PlanFS = mustSub("scaffold/plan")

// BuildFS is the build room's tree, written under build/ the first time it's used.
var BuildFS = mustSub("scaffold/build")

// AgentsFS holds the AGENTS.md fragments: root.md, and one <room>.md per room.
// They're assembled, never copied out as they are.
var AgentsFS = mustSub("scaffold/agents")

func mustSub(dir string) fs.FS {
	sub, err := fs.Sub(ScaffoldFS, dir)
	if err != nil {
		panic(err)
	}
	return sub
}
