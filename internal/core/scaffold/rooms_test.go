package scaffold

import (
	"io/fs"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/btdstudio/daiku/internal/core/settings"
)

// roomEngine is a daiku-shaped engine: a root tree, a plan and a build room, and
// the .daiku/ marker.
func roomEngine() Engine {
	return Engine{
		Tool: "daiku",
		FS: fstest.MapFS{
			"AGENTS.md":  {Data: []byte("agents\n")},
			".gitignore": {Data: []byte(".daiku/active\n")},
		},
		Rooms: map[string]fs.FS{
			"plan": fstest.MapFS{
				"context.md": {Data: []byte("plan room\n")},
			},
			"build": fstest.MapFS{
				"01_design/context.md": {Data: []byte("design room\n")},
				"tickets/.gitkeep":     {Data: nil},
				"personas/coder.md":    {Data: []byte("coder\n")},
			},
		},
		Marker: ".daiku",
		Seeds: func() ([]Seed, error) {
			return []Seed{{Rel: ".daiku/settings.json", Data: []byte("{}\n")}}, nil
		},
	}
}

func rootOnly(t *testing.T, e Engine) string {
	t.Helper()
	dir := t.TempDir()
	if _, err := e.Scaffold(dir, Options{NoGit: true}); err != nil {
		t.Fatal(err)
	}
	return dir
}

func exists(p string) bool {
	_, err := os.Stat(p)
	return err == nil
}

func TestInitWritesOnlyTheRoot(t *testing.T) {
	e := roomEngine()
	dir := rootOnly(t, e)
	for _, room := range []string{"plan", "build"} {
		if exists(filepath.Join(dir, room)) {
			t.Errorf("init created %s/", room)
		}
	}
	m, err := e.ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	for rel := range m.Files {
		if strings.HasPrefix(rel, "plan/") || strings.HasPrefix(rel, "build/") {
			t.Errorf("manifest lists room file %s before the room exists", rel)
		}
	}
	if !exists(filepath.Join(dir, ".daiku", "manifest.json")) {
		t.Error("no .daiku/manifest.json")
	}
}

// AC6: EnsureRoom writes the room under its folder, adds it to the one manifest,
// and a second call creates nothing.
func TestEnsureRoom(t *testing.T) {
	e := roomEngine()
	dir := rootOnly(t, e)
	before, err := e.ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}

	rep, err := e.EnsureRoom(dir, "build")
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"build/01_design/context.md", "build/personas/coder.md", "build/tickets/.gitkeep"}
	if !rep.Created || !reflect.DeepEqual(rep.Written, want) {
		t.Fatalf("report = %+v", rep)
	}
	for _, rel := range want {
		if !exists(filepath.Join(dir, filepath.FromSlash(rel))) {
			t.Errorf("%s not written", rel)
		}
	}
	if exists(filepath.Join(dir, "plan")) {
		t.Error("adding build created plan/")
	}
	after, err := e.ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	for rel, hash := range before.Files {
		if after.Files[rel] != hash {
			t.Errorf("root entry %s changed", rel)
		}
	}
	for _, rel := range want {
		if after.Files[rel] == "" {
			t.Errorf("manifest is missing %s", rel)
		}
	}
	if len(after.Files) != len(before.Files)+len(want) {
		t.Errorf("manifest has %d files, want %d", len(after.Files), len(before.Files)+len(want))
	}
	if got, _ := e.BuildManifest(dir); !reflect.DeepEqual(got.Files, after.Files) {
		t.Errorf("manifest after EnsureRoom differs from BuildManifest:\n got %v\nwant %v", after.Files, got.Files)
	}

	again, err := e.EnsureRoom(dir, "build")
	if err != nil {
		t.Fatal(err)
	}
	if again.Created || len(again.Written) != 0 {
		t.Fatalf("second call = %+v, want nothing created", again)
	}
	if last, _ := e.ReadManifest(dir); !reflect.DeepEqual(last, after) {
		t.Error("second call changed the manifest")
	}
}

func TestEnsureRoomLeavesAnExistingFolderAlone(t *testing.T) {
	e := roomEngine()
	dir := rootOnly(t, e)
	if err := os.MkdirAll(filepath.Join(dir, "plan"), 0o755); err != nil {
		t.Fatal(err)
	}
	rep, err := e.EnsureRoom(dir, "plan")
	if err != nil || rep.Created {
		t.Fatalf("rep = %+v, err = %v", rep, err)
	}
	if exists(filepath.Join(dir, "plan", "context.md")) {
		t.Error("EnsureRoom wrote into an existing room folder")
	}
}

func TestEnsureRoomRefusesUnknownRoomsAndNonWorkspaces(t *testing.T) {
	e := roomEngine()
	dir := rootOnly(t, e)
	if _, err := e.EnsureRoom(dir, "ideas"); err == nil || !strings.Contains(err.Error(), `unknown room "ideas"`) {
		t.Errorf("err = %v", err)
	}
	e.Rooms["../x"] = fstest.MapFS{"a.md": {Data: []byte("a")}}
	if _, err := e.EnsureRoom(dir, "../x"); err == nil || !strings.Contains(err.Error(), "invalid room name") {
		t.Errorf("err = %v", err)
	}
	empty := t.TempDir()
	if _, err := roomEngine().EnsureRoom(empty, "build"); err == nil || !strings.Contains(err.Error(), "not a daiku workspace") {
		t.Errorf("err = %v", err)
	}
	if exists(filepath.Join(empty, "build")) {
		t.Error("EnsureRoom wrote into a non-workspace")
	}
}

func TestEnsureRoomRebuildsAMissingManifest(t *testing.T) {
	e := roomEngine()
	dir := rootOnly(t, e)
	if err := os.Remove(filepath.Join(dir, ".daiku", "manifest.json")); err != nil {
		t.Fatal(err)
	}
	rep, err := e.EnsureRoom(dir, "plan")
	if err != nil || !rep.Created {
		t.Fatalf("rep = %+v, err = %v", rep, err)
	}
	m, err := e.ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	if m.Files["plan/context.md"] == "" || len(m.Files) != 1 {
		t.Errorf("manifest = %v, want only the room's file", m.Files)
	}
}

func TestEnsureRoomUnderVCSNone(t *testing.T) {
	e := roomEngine()
	e.VCS = settings.VCSNone
	dir := rootOnly(t, e)
	rep, err := e.EnsureRoom(dir, "build")
	if err != nil {
		t.Fatal(err)
	}
	for _, rel := range rep.Written {
		if strings.HasSuffix(rel, ".gitkeep") {
			t.Errorf("wrote %s under vcs none", rel)
		}
	}
	if info, err := os.Stat(filepath.Join(dir, "build", "tickets")); err != nil || !info.IsDir() {
		t.Errorf("build/tickets/ not created under vcs none: %v", err)
	}
}

func TestUpdateRefreshesExistingRoomsOnly(t *testing.T) {
	e := roomEngine()
	dir := rootOnly(t, e)
	if _, err := e.EnsureRoom(dir, "build"); err != nil {
		t.Fatal(err)
	}
	edited := filepath.Join(dir, "build", "personas", "coder.md")
	if err := os.WriteFile(edited, []byte("my coder\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	next := roomEngine()
	next.Rooms["build"] = fstest.MapFS{
		"01_design/context.md": {Data: []byte("design room v2\n")},
		"tickets/.gitkeep":     {Data: nil},
		"personas/coder.md":    {Data: []byte("coder v2\n")},
	}
	next.Rooms["plan"] = fstest.MapFS{"context.md": {Data: []byte("plan room v2\n")}}
	rep, err := next.Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !contains(rep.Updated, "build/01_design/context.md") {
		t.Errorf("updated = %v, want build/01_design/context.md", rep.Updated)
	}
	if !contains(rep.Skipped, "build/personas/coder.md") {
		t.Errorf("skipped = %v, want the edited build/personas/coder.md", rep.Skipped)
	}
	if got, _ := os.ReadFile(filepath.Join(dir, "build", "01_design", "context.md")); string(got) != "design room v2\n" {
		t.Errorf("room file = %q", got)
	}
	if got, _ := os.ReadFile(edited); string(got) != "my coder\n" {
		t.Errorf("edited file overwritten: %q", got)
	}
	if exists(filepath.Join(dir, "plan")) {
		t.Error("update created the plan room")
	}
}

func TestCheckCoversRooms(t *testing.T) {
	e := roomEngine()
	e.Rooms["plan"] = fstest.MapFS{"bad.md": {Data: []byte("hi {{ name }}")}}
	if got := e.Check(); len(got) != 1 || !strings.Contains(got[0], "plan/bad.md") {
		t.Fatalf("problems = %v", got)
	}
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}
