package layout

import (
	"strings"
	"testing"

	"github.com/btdstudio/daiku/internal/settings"
)

// Every plan folder is inside the plan room, and the settings defaults agree
// with the layout.
func TestLayout(t *testing.T) {
	for _, p := range []string{SpecsDir, IntakeDir, ADRDir, SyncFile, ResearchDir, ExploreDir} {
		if !strings.HasPrefix(p, Room+"/") {
			t.Errorf("%s isn't under %s/", p, Room)
		}
	}
	d := settings.DefaultSettings()
	if d.SourceDir != IntakeDir || d.AdrDir != ADRDir || d.SyncFile != SyncFile {
		t.Errorf("settings defaults %+v disagree with the layout", d)
	}
}
