package layout

import (
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/settings"
)

// Every plan folder is inside the plan room, sync.md is at the workspace root
// (shared with the build room), and the settings defaults agree with the layout.
func TestLayout(t *testing.T) {
	if SyncFile != "sync.md" {
		t.Errorf("SyncFile = %q, want the root sync.md", SyncFile)
	}
	for _, p := range []string{SpecsDir, IntakeDir, ADRDir, ResearchDir, ExploreDir} {
		if !strings.HasPrefix(p, Room+"/") {
			t.Errorf("%s isn't under %s/", p, Room)
		}
	}
	d := settings.DefaultSettings()
	if d.SourceDir != IntakeDir || d.AdrDir != ADRDir || d.SyncFile != SyncFile {
		t.Errorf("settings defaults %+v disagree with the layout", d)
	}
}
