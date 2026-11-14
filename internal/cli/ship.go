package cli

import (
	"errors"
	"fmt"
	"os"
	"path"
	"strings"

	"github.com/btdstudio/daiku/internal/build/ship"
	"github.com/btdstudio/daiku/internal/plan/layout"
	"github.com/btdstudio/daiku/internal/state"
)

// activeBuild is the active build change and its repo's local path. commit and pr
// act only on a build (design D8), and only in its registered git checkout.
func activeBuild(root string) (state.State, string, error) {
	st, err := state.LoadState(root)
	if err != nil {
		return state.State{}, "", err
	}
	if st.Name == "" || st.Loop != state.BuildLoop || len(st.Repos) == 0 {
		return state.State{}, "", errors.New("daiku: commit and pr need an active build — run `daiku build new <ticket-id>`")
	}
	dir, err := ship.RepoDir(root, st.Repos[0])
	if err != nil {
		return state.State{}, "", err
	}
	return st, dir, nil
}

// runCommit commits what's staged in the active build's repo, as tomo's did: it
// refuses when nothing is staged, so each commit is one reviewed group.
func runCommit(root string, args []string) int {
	_, dir, err := activeBuild(root)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	// `git diff --cached --quiet` exits 0 when nothing is staged.
	if _, err := ship.Output(dir, "git", "diff", "--cached", "--quiet"); err == nil {
		fmt.Fprintln(os.Stderr, "daiku commit: nothing staged — stage one logical change with `git add`, then commit it")
		return 1
	}
	if err := ship.Runner(dir, "git", append([]string{"commit"}, args...)...); err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	return 0
}

// runPR opens a pull request from the active build's repo, linking its ticket.
func runPR(root string, args []string) int {
	st, dir, err := activeBuild(root)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	t := changeTicket(root, st)
	if err := ship.Runner(dir, "gh", ship.PRArgs(dir, t, planFolder(st), args)...); err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	return 0
}

// planFolder is the plan change folder of a build whose ticket is planned, read
// in place from plan/specs/<change>/tickets/, or "" for an inbox ticket.
func planFolder(st state.State) string {
	for _, src := range st.Sources {
		if strings.HasPrefix(src, layout.SpecsDir+"/") && path.Base(path.Dir(src)) == "tickets" {
			return path.Dir(path.Dir(src))
		}
	}
	return ""
}
