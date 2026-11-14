package state

import (
	"path/filepath"
	"testing"
)

// planLoop is the loop the ported kiso tests run in: the sample machine, with
// change folders under specs/.
const planLoop = "plan"

// changeDir is a plan change's folder in the test store's layout.
func changeDir(root, name string) string { return filepath.Join(root, "specs", name) }

// specsPath is the test store's plan folder.
func specsPath(root string) string { return filepath.Join(root, "specs") }

// withLoop returns s with its plan loop changed by edit. A Loop is held by value
// in the map, so it can't be changed in place.
func withLoop(t *testing.T, s Store, edit func(l *Loop)) Store {
	t.Helper()
	loops := map[string]Loop{}
	for name, l := range s.Loops {
		loops[name] = l
	}
	l := loops[planLoop]
	edit(&l)
	loops[planLoop] = l
	s.Loops = loops
	return s
}
