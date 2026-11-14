package icm

import (
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

func scaffoldWorkspace(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	if _, err := Scaffold(dir, Options{NoGit: true}, ""); err != nil {
		t.Fatal(err)
	}
	return dir
}

func TestUpdateSkipsEditedAndRestoresMissing(t *testing.T) {
	dir := scaffoldWorkspace(t)
	agents := filepath.Join(dir, "AGENTS.md")
	if err := os.WriteFile(agents, []byte("mine"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(filepath.Join(dir, "README.md")); err != nil {
		t.Fatal(err)
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.Skipped) != 1 || rep.Skipped[0] != "AGENTS.md" {
		t.Fatalf("skipped = %v", rep.Skipped)
	}
	if len(rep.Updated) != 1 || rep.Updated[0] != "README.md" {
		t.Fatalf("updated = %v", rep.Updated)
	}
	if got, _ := os.ReadFile(agents); string(got) != "mine" {
		t.Fatalf("edit clobbered: %q", got)
	}
}

func TestUpdateReplacesUneditedFileFromOlderVersion(t *testing.T) {
	dir := scaffoldWorkspace(t)
	readme := filepath.Join(dir, "README.md")
	old := []byte("older scaffold")
	if err := os.WriteFile(readme, old, 0o644); err != nil {
		t.Fatal(err)
	}
	m, err := ReadManifest(dir)
	if err != nil {
		t.Fatal(err)
	}
	m.Files["README.md"] = hashBytes(old)
	if err := WriteManifest(dir, m); err != nil {
		t.Fatal(err)
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.Updated) != 1 || rep.Updated[0] != "README.md" {
		t.Fatalf("updated = %v", rep.Updated)
	}
	if got, _ := os.ReadFile(readme); string(got) == string(old) {
		t.Fatal("README.md was not refreshed")
	}
}

// A deleted .daiku/ must not cost the user their edits.
func TestUpdateRebuildsMissingManifestWithoutClobbering(t *testing.T) {
	dir := scaffoldWorkspace(t)
	// The manifest is lost; .daiku/ itself is the marker that says this is still
	// a workspace.
	if err := os.Remove(filepath.Join(dir, WorkspaceDir, "manifest.json")); err != nil {
		t.Fatal(err)
	}
	agents := filepath.Join(dir, "AGENTS.md")
	if err := os.WriteFile(agents, []byte("mine"), 0o644); err != nil {
		t.Fatal(err)
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.Skipped) != 1 || rep.Skipped[0] != "AGENTS.md" || len(rep.Updated) != 0 {
		t.Fatalf("rep = %+v", rep)
	}
	if got, _ := os.ReadFile(agents); string(got) != "mine" {
		t.Fatalf("edit clobbered: %q", got)
	}
	if _, err := ReadManifest(dir); err != nil {
		t.Fatalf("manifest not rebuilt: %v", err)
	}
}

func TestUpdateRefusesNonWorkspace(t *testing.T) {
	dir := t.TempDir()
	if _, err := Update(dir); err == nil || !strings.Contains(err.Error(), "not a daiku workspace") {
		t.Fatalf("want not a daiku workspace, got %v", err)
	}
	if entries, _ := os.ReadDir(dir); len(entries) != 0 {
		t.Fatalf("wrote into a non-workspace: %v", entries)
	}
}

func TestScaffoldSeedsStateAtRootAndIgnoresOnlyActive(t *testing.T) {
	dir := scaffoldWorkspace(t)
	for _, rel := range []string{"STATE.json", "LOG.md", ".gitattributes", ".daiku/settings.json", ".daiku/manifest.json"} {
		if _, err := os.Stat(filepath.Join(dir, filepath.FromSlash(rel))); err != nil {
			t.Fatalf("%s not seeded: %v", rel, err)
		}
	}
	ignore, err := os.ReadFile(filepath.Join(dir, ".gitignore"))
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(string(ignore), "\n")
	var active, whole bool
	for _, l := range lines {
		active = active || l == ".daiku/active"
		whole = whole || l == ".daiku/"
	}
	if !active || whole {
		t.Fatalf(".gitignore must ignore only .daiku/active:\n%s", ignore)
	}
}

func TestSyncFileEditsSurviveUpdate(t *testing.T) {
	dir := scaffoldWorkspace(t)
	if _, err := EnsurePlan(dir); err != nil {
		t.Fatal(err)
	}
	sync := filepath.Join(dir, "plan", "06_publish", "sync.md")
	if err := os.WriteFile(sync, []byte("my instructions"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := Update(dir); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(sync); string(got) != "my instructions" {
		t.Fatalf("sync.md clobbered: %q", got)
	}
	if _, err := os.Stat(filepath.Join(dir, "plan", "06_publish", "sync-examples", "github.md")); err != nil {
		t.Fatal(err)
	}
}

func TestUpdateAddsReferencesWithoutTouchingUserFiles(t *testing.T) {
	dir := scaffoldWorkspace(t)
	guide := filepath.Join(dir, "references", "context.md")
	if _, err := os.Stat(guide); err != nil {
		t.Fatalf("init didn't create references/context.md: %v", err)
	}
	// Make it look like a workspace from before the room existed.
	if err := os.RemoveAll(filepath.Join(dir, "references")); err != nil {
		t.Fatal(err)
	}
	user := map[string]string{
		"00_intake/1.md":                "an intake",
		"specs/1-x/functional-spec.md":  "a spec",
		"01_research/1-x/notes.md":      "research",
		"02_explore/1-x/findings.md":    "findings",
		"04_architecture/adr/0001-x.md": "an adr",
		"specs/1-x/tickets/S-0001.md":   "a ticket",
		"specs/1-x/README.md":           "an overview",
	}
	for rel, content := range user {
		p := filepath.Join(dir, filepath.FromSlash(rel))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !slices.Contains(rep.Updated, "references/context.md") {
		t.Fatalf("updated = %v, want references/context.md", rep.Updated)
	}
	if _, err := os.Stat(guide); err != nil {
		t.Fatalf("references/context.md wasn't added: %v", err)
	}
	if _, err := os.Stat(filepath.Join(dir, "references", "index.md")); err == nil {
		t.Error("update created the user's index.md")
	}
	for rel, content := range user {
		got, err := os.ReadFile(filepath.Join(dir, filepath.FromSlash(rel)))
		if err != nil || string(got) != content {
			t.Errorf("%s changed: %q, %v", rel, got, err)
		}
	}
}

// AC1: update adds the ideas room to an older workspace without touching a user file.
func TestUpdateAddsIdeasWithoutTouchingUserFiles(t *testing.T) {
	dir := scaffoldWorkspace(t)
	ideaFiles := []string{"ideas/context.md", "ideas/personas/facilitator.md", "ideas/templates/summary.md", "ideas/templates/log-entry.md"}
	for _, rel := range ideaFiles {
		if _, err := os.Stat(filepath.Join(dir, filepath.FromSlash(rel))); err != nil {
			t.Fatalf("init didn't create %s: %v", rel, err)
		}
	}
	if err := os.RemoveAll(filepath.Join(dir, "ideas")); err != nil {
		t.Fatal(err)
	}
	user := map[string]string{
		"00_intake/1.md":                "an intake",
		"specs/1-x/functional-spec.md":  "a spec",
		"references/index.md":           "my index",
		"references/notes.md":           "a note",
		"04_architecture/adr/0001-x.md": "an adr",
	}
	for rel, content := range user {
		p := filepath.Join(dir, filepath.FromSlash(rel))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	rep, err := Update(dir)
	if err != nil {
		t.Fatal(err)
	}
	for _, rel := range ideaFiles {
		if !slices.Contains(rep.Updated, rel) {
			t.Errorf("updated = %v, want %s", rep.Updated, rel)
		}
	}
	for rel, content := range user {
		if got, err := os.ReadFile(filepath.Join(dir, filepath.FromSlash(rel))); err != nil || string(got) != content {
			t.Errorf("%s changed: %q, %v", rel, got, err)
		}
	}
}
