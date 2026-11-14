// Package settings defines daiku's settings shape and merges it through the
// core: effective settings = defaults ← global (~/.config/daiku/settings.json) ←
// project (.daiku/settings.json).
package settings

import (
	"fmt"
	"path/filepath"

	core "github.com/btdstudio/daiku/internal/core/settings"
)

// Settings is the effective settings shape.
type Settings struct {
	BaseSource string `json:"baseSource"`
	Gates      string `json:"gates"`
	SourceDir  string `json:"sourceDir"`
	AdrDir     string `json:"adrDir"`
	SyncFile   string `json:"syncFile"`
	Overview   string `json:"overview"`
}

// Loaded is the result of LoadSettings.
type Loaded = core.Loaded[Settings]

var spec = core.Spec[Settings]{
	Tool:     "daiku",
	Defaults: DefaultSettings,
	Apply:    apply,
	Validate: func(s *Settings) error {
		if err := core.CheckGates("daiku", s.Gates); err != nil {
			return err
		}
		// A committed settings file must not point a store outside the workspace.
		for name, dir := range map[string]string{"sourceDir": s.SourceDir, "adrDir": s.AdrDir, "syncFile": s.SyncFile} {
			if dir != "" && !filepath.IsLocal(dir) {
				return fmt.Errorf("daiku: %s %q must be a path inside the workspace", name, dir)
			}
		}
		return checkOverview(s.Overview)
	},
}

// The overview setting's values: auto regenerates each change's README on gates
// and ticket commands; manual leaves it to `daiku overview`.
const (
	OverviewAuto   = "auto"
	OverviewManual = "manual"
)

func checkOverview(v string) error {
	if v == OverviewAuto || v == OverviewManual {
		return nil
	}
	return fmt.Errorf("daiku: invalid overview %q (expected auto|manual)", v)
}

func DefaultSettings() Settings {
	return Settings{
		BaseSource: "",                         // default base source: a plan/00_intake path | "issue:N" | "url:..."
		Gates:      "block",                    // block | prompt | auto
		SourceDir:  "plan/00_intake",           // the intake store
		AdrDir:     "plan/04_architecture/adr", // architecture decision records
		SyncFile:   "sync.md",                  // how an agent talks to the PM tool, for both rooms
		Overview:   OverviewAuto,               // auto | manual: regenerate specs/<change>/README.md
	}
}

func apply(s *Settings, key string, value any) {
	val, ok := value.(string)
	if !ok {
		return
	}
	switch key {
	case "baseSource":
		s.BaseSource = val
	case "gates":
		s.Gates = val
	case "sourceDir":
		s.SourceDir = val
	case "adrDir":
		s.AdrDir = val
	case "syncFile":
		s.SyncFile = val
	case "overview":
		s.Overview = val
	}
}

func GlobalSettingsPath() string { return spec.GlobalPath() }

func ProjectSettingsPath(root string) string { return spec.ProjectPath(root) }

// ReadIfExists parses a JSON object file, returning an empty map when absent.
func ReadIfExists(file string) (map[string]any, error) { return spec.ReadIfExists(file) }

// MergeSettings overlays the raw layers (global then project) over defaults.
func MergeSettings(layers ...map[string]any) (Settings, error) { return spec.Merge(layers...) }

func LoadSettings(root string) (Loaded, error) { return spec.Load(root) }

// InitVCS is the "vcs" value for a workspace that `daiku init` is about to create.
// --no-git forces "none"; otherwise the global settings decide, defaulting to
// "git". There are no project settings yet to consult.
func InitVCS(noGit bool) (string, error) {
	if noGit {
		return core.VCSNone, nil
	}
	global, err := ReadIfExists(GlobalSettingsPath())
	if err != nil {
		return "", err
	}
	return core.VCSOf("daiku", global)
}
