package icm

import (
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"testing"
)

func exists(p string) bool {
	_, err := os.Stat(p)
	return err == nil
}

func agentsText(t *testing.T, dir string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(dir, "AGENTS.md"))
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

// AC1: init in an empty folder writes only the root: the root files, .daiku/,
// ideas/ and references/, and no plan/ or build/.
func TestScaffoldIsRootOnly(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	dir := t.TempDir()
	if _, err := Scaffold(dir, Options{NoGit: true}, "none"); err != nil {
		t.Fatal(err)
	}
	for _, rel := range []string{"AGENTS.md", "CLAUDE.md", "README.md", "STATE.json", "LOG.md", "REPOS.json", ".daiku", "ideas", "references"} {
		if !exists(filepath.Join(dir, rel)) {
			t.Errorf("init didn't write %s", rel)
		}
	}
	for _, rel := range []string{"plan", "build", "specs", "00_intake"} {
		if exists(filepath.Join(dir, rel)) {
			t.Errorf("init wrote %s", rel)
		}
	}
	text := agentsText(t, dir)
	if !strings.Contains(text, "## How to start") || strings.Contains(text, "## Plan loop") {
		t.Errorf("root-only AGENTS.md:\n%s", text)
	}
	state, _ := os.ReadFile(filepath.Join(dir, "STATE.json"))
	if !strings.Contains(string(state), `"schema": 1`) {
		t.Errorf("STATE.json = %s", state)
	}
}

// AC3: EnsurePlan creates plan/ and adds its AGENTS.md sections; a second call
// creates nothing.
func TestEnsurePlan(t *testing.T) {
	dir := scaffoldWorkspace(t)
	created, err := EnsurePlan(dir)
	if err != nil || !created {
		t.Fatalf("created = %v, err = %v", created, err)
	}
	if !exists(filepath.Join(dir, "plan", "00_intake", "context.md")) || !exists(filepath.Join(dir, "plan", "specs")) {
		t.Error("plan room files missing")
	}
	if exists(filepath.Join(dir, "build")) {
		t.Error("EnsurePlan created build/")
	}
	if text := agentsText(t, dir); !strings.Contains(text, "## Plan loop") || !strings.Contains(text, "## How to start") {
		t.Errorf("AGENTS.md lacks the plan sections:\n%s", text)
	}
	m, err := ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	if m.Files["plan/00_intake/context.md"] == "" {
		t.Error("manifest doesn't list the plan room")
	}
	before := agentsText(t, dir)
	again, err := EnsurePlan(dir)
	if err != nil || again {
		t.Fatalf("second EnsurePlan = %v, %v", again, err)
	}
	if agentsText(t, dir) != before {
		t.Error("second EnsurePlan changed AGENTS.md")
	}
}

// AC4: update keeps a section the person added and refreshes daiku's sections
// for the rooms that exist.
func TestUpdateKeepsTheirSectionAndRefreshesRooms(t *testing.T) {
	dir := scaffoldWorkspace(t)
	if _, err := EnsurePlan(dir); err != nil {
		t.Fatal(err)
	}
	text := agentsText(t, dir)
	text = strings.Replace(text, "## Plan routing\n", "## Plan routing\n\nstale rows\n", 1)
	text += "\n## Team notes\n\nkeep me\n"
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	got := agentsText(t, dir)
	if !strings.Contains(got, "## Team notes\n\nkeep me\n") {
		t.Error("their section was lost")
	}
	if strings.Contains(got, "stale rows") {
		t.Error("Plan routing wasn't refreshed")
	}
	if !slices.Contains(rep.Merged, "AGENTS.md") {
		t.Errorf("report = %+v, want AGENTS.md merged", rep)
	}
	again, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Contains(again.Unchanged, "AGENTS.md") || agentsText(t, dir) != got {
		t.Errorf("a second update changed AGENTS.md: %+v", again)
	}
}

// A file the merge can't handle is left alone and reported, not overwritten.
func TestUpdateReportsUnmergeableAgents(t *testing.T) {
	dir := scaffoldWorkspace(t)
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte("no headings here\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if rep.NotMerged["AGENTS.md"] != "no ## headings" || !slices.Contains(rep.Skipped, "AGENTS.md") {
		t.Errorf("report = %+v", rep)
	}
	if agentsText(t, dir) != "no headings here\n" {
		t.Error("an unmergeable AGENTS.md was overwritten")
	}
}

// A deleted AGENTS.md comes back on update.
func TestUpdateRestoresMissingAgents(t *testing.T) {
	dir := scaffoldWorkspace(t)
	if err := os.Remove(filepath.Join(dir, "AGENTS.md")); err != nil {
		t.Fatal(err)
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Contains(rep.Updated, "AGENTS.md") || !exists(filepath.Join(dir, "AGENTS.md")) {
		t.Errorf("report = %+v", rep)
	}
}

// R10: init and update refuse a zumen or tomo workspace and write nothing.
func TestLegacyWorkspacesAreRefused(t *testing.T) {
	for _, tool := range []string{"zumen", "tomo"} {
		dir := t.TempDir()
		if err := os.Mkdir(filepath.Join(dir, "."+tool), 0o755); err != nil {
			t.Fatal(err)
		}
		if _, err := Scaffold(dir, Options{NoGit: true}, "none"); err == nil || !strings.Contains(err.Error(), "a "+tool+" workspace") {
			t.Errorf("%s: Scaffold err = %v", tool, err)
		}
		if _, err := Update(dir); err == nil || !strings.Contains(err.Error(), "a "+tool+" workspace") {
			t.Errorf("%s: Update err = %v", tool, err)
		}
		if entries, _ := os.ReadDir(dir); len(entries) != 1 {
			t.Errorf("%s: wrote %d entries", tool, len(entries)-1)
		}
	}
}

// AC10: a symlinked references/ or AGENTS.md is refused by update, and nothing
// is written where it points.
func TestUpdateRefusesSymlinks(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlinks need privileges on Windows")
	}
	t.Run("references", func(t *testing.T) {
		dir := scaffoldWorkspace(t)
		outside := t.TempDir()
		if err := os.RemoveAll(filepath.Join(dir, "references")); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, filepath.Join(dir, "references")); err != nil {
			t.Fatal(err)
		}
		if _, err := Update(dir); err == nil || !strings.Contains(err.Error(), "references is a symlink") {
			t.Fatalf("err = %v", err)
		}
		if entries, _ := os.ReadDir(outside); len(entries) != 0 {
			t.Fatalf("wrote %d entries outside", len(entries))
		}
	})
	t.Run("AGENTS.md", func(t *testing.T) {
		dir := scaffoldWorkspace(t)
		outside := filepath.Join(t.TempDir(), "AGENTS.md")
		if err := os.WriteFile(outside, []byte("theirs\n"), 0o644); err != nil {
			t.Fatal(err)
		}
		if err := os.Remove(filepath.Join(dir, "AGENTS.md")); err != nil {
			t.Fatal(err)
		}
		if err := os.Symlink(outside, filepath.Join(dir, "AGENTS.md")); err != nil {
			t.Fatal(err)
		}
		if _, err := Update(dir); err == nil || !strings.Contains(err.Error(), "AGENTS.md is a symlink") {
			t.Fatalf("err = %v", err)
		}
		if got, _ := os.ReadFile(outside); string(got) != "theirs\n" {
			t.Fatalf("the file outside was changed: %q", got)
		}
	})
}

// Code review finding 2: init over an existing AGENTS.md it can't merge leaves
// the file alone, reports it, and succeeds.
func TestScaffoldSkipsAnUnmergeableAgents(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte("no headings here\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	rep, err := Scaffold(dir, Options{NoGit: true}, "none")
	if err != nil {
		t.Fatalf("Scaffold = %v", err)
	}
	if rep.NotMerged["AGENTS.md"] != "no ## headings" || slices.Contains(rep.Written, "AGENTS.md") {
		t.Errorf("report = %+v", rep)
	}
	if agentsText(t, dir) != "no headings here\n" {
		t.Error("init overwrote an AGENTS.md it couldn't merge")
	}
	if !exists(filepath.Join(dir, "STATE.json")) {
		t.Error("init stopped before the rest of the root")
	}
}
