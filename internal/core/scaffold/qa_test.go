package scaffold

import (
	"strings"
	"testing"
)

// QA (AC6): adding both rooms, in either order, ends with one manifest holding
// the root and both rooms, and each room stays idempotent.
func TestQABothRoomsShareOneManifest(t *testing.T) {
	for _, order := range [][]string{{"build", "plan"}, {"plan", "build"}} {
		e := roomEngine()
		dir := rootOnly(t, e)
		for _, room := range order {
			if rep, err := e.EnsureRoom(dir, room); err != nil || !rep.Created {
				t.Fatalf("%v: EnsureRoom(%s) = %+v, %v", order, room, rep, err)
			}
		}
		for _, room := range order {
			if rep, err := e.EnsureRoom(dir, room); err != nil || rep.Created || len(rep.Written) != 0 {
				t.Fatalf("%v: second EnsureRoom(%s) = %+v, %v", order, room, rep, err)
			}
		}
		m, err := e.ReadManifest(dir)
		if err != nil {
			t.Fatal(err)
		}
		counts := map[string]int{}
		for rel := range m.Files {
			switch {
			case strings.HasPrefix(rel, "plan/"):
				counts["plan"]++
			case strings.HasPrefix(rel, "build/"):
				counts["build"]++
			default:
				counts["root"]++
			}
		}
		if counts["plan"] != 1 || counts["build"] != 3 || counts["root"] == 0 {
			t.Errorf("%v: manifest counts = %v", order, counts)
		}
		if got, _ := e.BuildManifest(dir); len(got.Files) != len(m.Files) {
			t.Errorf("%v: manifest has %d files, BuildManifest %d", order, len(m.Files), len(got.Files))
		}
	}
}
