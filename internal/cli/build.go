package cli

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/btdstudio/daiku/internal/build/inbox"
	"github.com/btdstudio/daiku/internal/core/repos"
	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/icm"
	"github.com/btdstudio/daiku/internal/state"
)

// runBuild handles `daiku build`: only `new` so far.
func runBuild(root string, args []string) int {
	if len(args) == 0 || args[0] != "new" {
		fmt.Fprint(os.Stderr, usageText("build"))
		return 1
	}
	if err := buildNew(root, args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	return 0
}

// buildNew creates a build change for one ticket (design D3): it finds the
// ticket where it lives, checks it can be built, creates the build room the
// first time, and seeds the change named by the ticket's id in its repo. The
// ticket is never copied.
func buildNew(root string, args []string) error {
	if len(args) != 1 || strings.HasPrefix(args[0], "-") {
		return errors.New(usageLine("build", "new"))
	}
	id := inbox.NormalizeID(args[0])
	if !ticket.IDRe.MatchString(id) {
		return fmt.Errorf("daiku: %q isn't a ticket id", args[0])
	}
	changes, err := state.ReadChanges(root)
	if err != nil {
		return err
	}
	if changes[id] != nil {
		return fmt.Errorf("daiku: %s is already a change — run `daiku switch %s`", id, id)
	}
	t, rel, err := findBuildTicket(root, id)
	if err != nil {
		return err
	}
	if err := checkBuildable(root, t); err != nil {
		return err
	}
	// Refuse unsafe state files, and a plan overview that can't be written
	// safely, before the room or the change folder is written.
	if err := state.CheckStateFiles(root); err != nil {
		return err
	}
	if err := state.CheckOverview(root, state.State{Name: id, Loop: state.BuildLoop, Sources: []string{rel}}); err != nil {
		return err
	}
	created, err := icm.EnsureBuild(root)
	if err != nil {
		return err
	}
	if created {
		fmt.Println("created room build/")
	}
	if _, err := state.SeedBuild(root, id, t.Repo, rel); err != nil {
		return err
	}
	fmt.Printf("created build %s for %s (%s)\n", id, t.Repo, rel)
	return nil
}

// findBuildTicket finds ticket id in a plan change's tickets/ folder, then in the
// inbox. It returns the ticket and its path relative to root. An id found in
// both places, or a planned id in the inbox, is an error.
func findBuildTicket(root, id string) (*ticket.Ticket, string, error) {
	var found []string
	plans, err := state.PlanChanges(root)
	if err != nil {
		return nil, "", err
	}
	for _, c := range plans {
		if p := filepath.Join(ticketDir(root, c.Name), id+".md"); isFile(p) {
			found = append(found, p)
		}
	}
	if p := inbox.Path(root, id); isFile(p) {
		if plannedID(id) {
			return nil, "", fmt.Errorf("daiku: %s is a planned ticket in the inbox — planned tickets live under plan/specs/", id)
		}
		found = append(found, p)
	}
	switch len(found) {
	case 0:
		return nil, "", fmt.Errorf("daiku: no ticket %s in plan/specs/*/tickets/ or build/tickets/", id)
	case 1:
	default:
		var rels []string
		for _, p := range found {
			rels = append(rels, relTo(root, p))
		}
		return nil, "", fmt.Errorf("daiku: ticket %s is in more than one place: %s", id, strings.Join(rels, ", "))
	}
	data, err := os.ReadFile(found[0])
	if err != nil {
		return nil, "", err
	}
	rel := relTo(root, found[0])
	t, err := ticket.Parse(data)
	if err != nil {
		return nil, "", fmt.Errorf("daiku: %s: %v", rel, err)
	}
	t.Path = found[0]
	if t.ID != id {
		return nil, "", fmt.Errorf("daiku: %s holds ticket %q, not %s", rel, t.ID, id)
	}
	return t, rel, nil
}

// checkBuildable refuses a ticket that can't be built: one with no repo or an
// unregistered one, one that fails the schema, and a planned ticket that hasn't
// been readied or published, as tomo refused one.
func checkBuildable(root string, t *ticket.Ticket) error {
	if problems := ticket.ValidateFile(t); len(problems) > 0 {
		return fmt.Errorf("daiku: invalid ticket: %s", problems[0])
	}
	if plannedID(t.ID) && t.Status != ticket.StatusReady && t.Status != ticket.StatusPublished {
		return fmt.Errorf("daiku: %s is %s; only a ready or published planned ticket can be built", t.ID, t.Status)
	}
	if t.Repo == "" {
		return fmt.Errorf("daiku: %s has no repo — a build needs one", t.ID)
	}
	reg, err := repos.Load(root, "daiku")
	if err != nil {
		return err
	}
	if _, ok := reg.Repos[t.Repo]; !ok {
		return fmt.Errorf("daiku: %s's repo %q isn't registered — run `daiku repo add %s <remote>`", t.ID, t.Repo, t.Repo)
	}
	return nil
}

// plannedID reports whether id is a planned ticket's (E-, F-, S- or T-).
func plannedID(id string) bool {
	prefix, _, _ := strings.Cut(id, "-")
	return prefix != ticket.NativePrefix && prefix != ticket.PulledPrefix
}

func isFile(p string) bool {
	info, err := os.Stat(p)
	return err == nil && info.Mode().IsRegular()
}

// relTo is p relative to root, slash-separated.
func relTo(root, p string) string {
	rel, err := filepath.Rel(root, p)
	if err != nil {
		return filepath.ToSlash(p)
	}
	return filepath.ToSlash(rel)
}
