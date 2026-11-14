// Package layout names the plan room's folders, so every plan package agrees on
// where things live. It imports nothing from daiku but the core state's plan
// folder, and every plan package may import it.
package layout

import "github.com/btdstudio/daiku/internal/core/state"

const (
	// Room is the plan room's folder at the workspace root.
	Room = "plan"
	// SpecsDir holds one folder per plan change: plan/specs/<n>-<slug>/.
	SpecsDir = state.PlanDir
	// IntakeDir is the default intake store (settings sourceDir).
	IntakeDir = "plan/00_intake"
	// ADRDir is the default ADR folder (settings adrDir).
	ADRDir = "plan/04_architecture/adr"
	// SyncFile is the default sync instructions (settings syncFile).
	SyncFile = "plan/06_publish/sync.md"
	// ResearchDir and ExploreDir hold a per-change folder each, committed with
	// the change's gates.
	ResearchDir = "plan/01_research"
	ExploreDir  = "plan/02_explore"
)
