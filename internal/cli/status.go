package cli

import (
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/btdstudio/daiku/internal/build/lock"
	"github.com/btdstudio/daiku/internal/build/ship"
	"github.com/btdstudio/daiku/internal/core/repos"
	"github.com/btdstudio/daiku/internal/settings"
	"github.com/btdstudio/daiku/internal/state"
)

// runStatus prints the active change's block and the board of every open change
// (S-0042 D6, D8), or with a plan change's name, that plan's tickets and their
// builds (D7). It only reads: no file is written and git never runs.
func runStatus(root string, args []string) int {
	switch {
	case len(args) == 1 && !strings.HasPrefix(args[0], "-"):
		return runInitiative(root, args[0])
	case len(args) > 0:
		fmt.Fprintln(os.Stderr, usageLine("status"))
		return 1
	}
	loaded, err := settings.LoadSettings(root)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	st, err := state.LoadState(root)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	changes, err := state.ReadChanges(root)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	if st.Name == "" {
		fmt.Println("no active change")
	} else {
		printActive(root, st)
	}
	fmt.Printf("vcs:      %s\n", loaded.VCS)
	if st.Name != "" {
		warnSkippedExplore(root, st)
	}
	fmt.Print(board(changes, st.Name))
	return 0
}

// printActive is the active change's block, without its vcs line.
func printActive(root string, st state.State) {
	fmt.Printf("change:   %s\n", st.Name)
	fmt.Printf("room:     %s\n", st.Loop)
	if len(st.Repos) > 0 {
		fmt.Printf("repo:     %s\n", strings.Join(st.Repos, ", "))
	}
	fmt.Printf("phase:    %s\n", st.Phase)
	fmt.Printf("status:   %s\n", st.Status)
	var gates []string
	for _, p := range state.MachineFor(st.Loop).Phases {
		gates = append(gates, p.Name+"="+st.Gates[p.Name])
	}
	fmt.Printf("gates:    %s\n", strings.Join(gates, " "))
	fmt.Printf("initiative: %s\n", initiative(root, st))
	if len(st.Repos) > 0 {
		fmt.Printf("code:     %s\n", codePaths(root, st.Repos))
	}
	if st.Loop == state.BuildLoop {
		// tomo's ticket and branch lines: the branch ship expects.
		if t := changeTicket(root, st); t != nil {
			fmt.Printf("ticket:   %s %s\n", t.ID, t.Title)
			fmt.Printf("branch:   %s\n", ship.BranchName(t))
		}
	}
}

// initiative is the plan a change belongs to: a plan itself; a build's plan when
// it's here, or its ticket's change marked as elsewhere; "-" for none (R2).
func initiative(root string, st state.State) string {
	if st.Loop != state.BuildLoop {
		return st.Name
	}
	if p := state.PlanOf(root, st); p != "" {
		return p
	}
	if t := changeTicket(root, st); t != nil && t.Change != "" {
		return t.Change + " (not in this workspace)"
	}
	return "-"
}

// codePaths is each repo's local checkout, "<repo>=<path>", or "(not set)".
func codePaths(root string, names []string) string {
	reg, err := repos.Load(root, "daiku")
	var parts []string
	for _, n := range names {
		path := "(not set)"
		if err == nil {
			if p, perr := reg.Resolve(n); perr == nil {
				path = p
			}
		}
		parts = append(parts, n+"="+path)
	}
	return strings.Join(parts, " ")
}

// board lists every change that isn't closed: plans by name, then builds grouped
// by repo with each repo's lock holder. active is marked "* ". It starts with a
// blank line, and says so when there are no changes at all.
func board(changes map[string]*state.State, active string) string {
	var plans []string
	builds := map[string][]string{}
	for name, c := range changes {
		if c == nil || c.IsClosed() {
			continue
		}
		if c.Loop == state.BuildLoop {
			repo := "(no repo)"
			if len(c.Repos) > 0 {
				repo = c.Repos[0]
			}
			builds[repo] = append(builds[repo], name)
			continue
		}
		plans = append(plans, name)
	}
	var b strings.Builder
	b.WriteString("\n")
	if len(changes) == 0 {
		b.WriteString("no changes yet\n")
		return b.String()
	}
	mark := func(name string) string {
		if name == active {
			return "* "
		}
		return "  "
	}
	if len(plans) > 0 {
		sort.Strings(plans)
		b.WriteString("plans:\n")
		for _, name := range plans {
			c := changes[name]
			fmt.Fprintf(&b, "%s%s  %s %s\n", mark(name), name, c.Phase, c.Status)
		}
	}
	if len(builds) > 0 {
		if len(plans) > 0 {
			b.WriteString("\n")
		}
		b.WriteString("builds:\n")
		var repoNames []string
		for r := range builds {
			repoNames = append(repoNames, r)
		}
		sort.Strings(repoNames)
		for _, r := range repoNames {
			ids := builds[r]
			sort.Strings(ids)
			holder := ""
			for _, id := range ids {
				if lock.Holds(*changes[id]) {
					holder = id
					break
				}
			}
			if holder != "" {
				fmt.Fprintf(&b, "  %s  held by %s\n", r, holder)
			} else {
				fmt.Fprintf(&b, "  %s  free\n", r)
			}
			for _, id := range ids {
				c := changes[id]
				done := ""
				if c.Phase == "ship" && c.Status == "complete" {
					done = " (done)"
				}
				fmt.Fprintf(&b, "  %s%s  %s %s%s\n", mark(id), id, c.Phase, c.Status, done)
			}
		}
	}
	if len(plans) == 0 && len(builds) == 0 {
		b.WriteString("no open changes\n")
	}
	return b.String()
}

// runInitiative prints a plan's phase, then one line per ticket: its id, repo and
// build (R5). Anything but a plan change in this workspace is refused (R6).
func runInitiative(root, name string) int {
	changes, err := state.ReadChanges(root)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	ok, err := state.IsPlan(root, name)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	if !ok {
		msg := fmt.Sprintf("daiku: %s isn't a plan change in this workspace", name)
		if c := changes[name]; c != nil && c.Loop == state.BuildLoop {
			msg += " (it's a build — bare daiku status lists it)"
		}
		fmt.Fprintln(os.Stderr, msg)
		return 1
	}
	rows, err := state.BuildRows(root, name)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	c := changes[name]
	fmt.Printf("%s  %s %s\n", name, c.Phase, c.Status)
	if len(rows) == 0 {
		fmt.Println("  no tickets yet")
	}
	for _, r := range rows {
		repo := r.Repo
		if repo == "" {
			repo = "-"
		}
		fmt.Printf("  %s  %s  %s\n", r.ID, repo, r.Build())
	}
	return 0
}
