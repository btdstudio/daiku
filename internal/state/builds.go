package state

import (
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/core/workspace"
	"github.com/btdstudio/daiku/internal/plan/layout"
	"github.com/btdstudio/daiku/internal/plan/overview"
	"github.com/btdstudio/daiku/internal/settings"
)

// BuildTicket reads a build's ticket from its first parsable source, or nil.
func BuildTicket(root string, c State) *ticket.Ticket {
	for _, src := range c.Sources {
		path := filepath.Join(root, filepath.FromSlash(src))
		data, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		if t, err := ticket.Parse(data); err == nil && t.ID != "" {
			t.Path = path
			return t
		}
	}
	return nil
}

// PlanOf is the plan change a build belongs to, or "" when it belongs to none in
// this workspace. A ticket under plan/specs/<plan>/tickets/ belongs to that plan;
// otherwise its change field names it. Either way the plan must be a recorded
// plan change whose folder exists (S-0042 D1).
func PlanOf(root string, c State) string {
	changes, err := ReadChanges(root)
	if err != nil {
		return ""
	}
	return planOf(root, changes, c)
}

func planOf(root string, changes map[string]*State, c State) string {
	if c.Loop != BuildLoop {
		return ""
	}
	name := ""
	for _, src := range c.Sources {
		rest, ok := strings.CutPrefix(src, layout.SpecsDir+"/")
		if parts := strings.Split(rest, "/"); ok && len(parts) == 3 && parts[1] == "tickets" {
			name = parts[0]
			break
		}
	}
	if name == "" {
		if t := BuildTicket(root, c); t != nil {
			name = t.Change
		}
	}
	if !isPlan(root, changes, name) {
		return ""
	}
	return name
}

// isPlan reports whether name is a recorded plan change whose folder is here.
func isPlan(root string, changes map[string]*State, name string) bool {
	if name == "" || !filepath.IsLocal(name) || strings.ContainsAny(name, `/\`) {
		return false
	}
	if c := changes[name]; c == nil || c.Loop != PlanLoop {
		return false
	}
	info, err := os.Stat(ChangeDir(root, name))
	return err == nil && info.IsDir()
}

// IsPlan reports whether name is a plan change in this workspace: recorded,
// open or closed, with its folder under plan/specs/.
func IsPlan(root, name string) (bool, error) {
	changes, err := ReadChanges(root)
	if err != nil {
		return false, err
	}
	return isPlan(root, changes, name), nil
}

// BuildRows lists a plan's tickets and their builds, sorted by id: every ticket
// in plan/specs/<plan>/tickets/ with a repo (epics and features are never
// built), and every other build whose PlanOf is the plan.
// A ticket's build is the build change named by its id (S-0042 D2).
func BuildRows(root, plan string) ([]overview.BuildRow, error) {
	changes, err := ReadChanges(root)
	if err != nil {
		return nil, err
	}
	ts, _, err := ticket.LoadDir(filepath.Join(ChangeDir(root, plan), "tickets"))
	if err != nil {
		return nil, err
	}
	rows := map[string]overview.BuildRow{}
	for _, t := range ts {
		if _, dup := rows[t.ID]; !dup && t.Repo != "" {
			rows[t.ID] = overview.BuildRow{ID: t.ID, Repo: t.Repo}
		}
	}
	for name, c := range changes {
		if c == nil || c.Loop != BuildLoop {
			continue
		}
		if _, ok := rows[name]; ok || planOf(root, changes, *c) != plan {
			continue
		}
		row := overview.BuildRow{ID: name}
		if len(c.Repos) > 0 {
			row.Repo = c.Repos[0]
		}
		rows[name] = row
	}
	out := make([]overview.BuildRow, 0, len(rows))
	for id, row := range rows {
		if c := changes[id]; c != nil && c.Loop == BuildLoop {
			row.Started = true
			row.Phase, row.Status, row.Closed = c.Phase, c.Status, c.IsClosed()
		}
		out = append(out, row)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].ID < out[j].ID })
	return out, nil
}

// overviewTarget is the plan whose README a transition of c rewrites: c itself
// for a plan, PlanOf(c) for a build, or "".
func overviewTarget(root string, c State) string {
	if c.Loop == BuildLoop {
		return PlanOf(root, c)
	}
	return c.Name
}

// CheckOverview refuses, before any state is written, a transition of c whose
// overview couldn't be written safely: a README outside the workspace or reached
// through a symlink. It's nil under `overview: manual` and when c has no plan
// (S-0042 D5).
func CheckOverview(root string, c State) error {
	loaded, err := settings.LoadSettings(root)
	if err != nil {
		return err
	}
	if loaded.Effective.Overview != settings.OverviewAuto {
		return nil
	}
	plan := overviewTarget(root, c)
	if plan == "" {
		return nil
	}
	return workspace.CheckPath(root, filepath.Join(root, filepath.FromSlash(OverviewPath(plan))))
}
