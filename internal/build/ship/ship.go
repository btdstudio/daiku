// Package ship is the build loop's last step, ported from tomo (af54ad2): the
// branch name and pull-request body for a build's ticket, and running git and gh
// in the build's repo. Commands run without a shell, through Runner and Output,
// which tests replace to record what would run.
package ship

import (
	"fmt"
	"os"
	"os/exec"
	"sort"
	"strings"

	"github.com/btdstudio/daiku/internal/core/repos"
	core "github.com/btdstudio/daiku/internal/core/state"
	"github.com/btdstudio/daiku/internal/core/ticket"
)

// Runner runs a command in dir with the person's terminal attached, as tomo's run
// did. It never goes through a shell.
var Runner = func(dir, name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin
	return cmd.Run()
}

// Output runs a command in dir and returns its trimmed standard output.
var Output = func(dir, name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	out, err := cmd.Output()
	return strings.TrimSpace(string(out)), err
}

// RepoDir is repo's local path from .daiku/paths.json. It must be a directory
// inside a git work tree, so commit and pr never run anywhere else.
func RepoDir(root, repo string) (string, error) {
	reg, err := repos.Load(root, "daiku")
	if err != nil {
		return "", err
	}
	dir, err := reg.Resolve(repo)
	if err != nil {
		return "", err
	}
	unusable := fmt.Errorf("daiku: %s has no usable local path (%s) — run `daiku repo path %s <dir>`", repo, dir, repo)
	if info, err := os.Stat(dir); err != nil || !info.IsDir() {
		return "", unusable
	}
	if out, err := Output(dir, "git", "rev-parse", "--is-inside-work-tree"); err != nil || out != "true" {
		return "", fmt.Errorf("daiku: %s's local path %s isn't a git checkout — run `daiku repo path %s <dir>`", repo, dir, repo)
	}
	return dir, nil
}

// BranchName is the ship branch for a ticket: its lower-cased id and the title's
// slug.
func BranchName(t *ticket.Ticket) string {
	name := strings.ToLower(t.ID)
	if slug := core.Slugify(t.Title); slug != "" {
		name += "-" + slug
	}
	return name
}

// PRBody links a pull request to its ticket: id and title, each external item,
// where it was planned, the plan change's folder for a planned ticket read in
// place, the spec and the ADRs.
func PRBody(t *ticket.Ticket, planFolder string) string {
	lines := []string{fmt.Sprintf("Implements %s: %s", t.ID, t.Title), ""}
	tools := make([]string, 0, len(t.External))
	for tool := range t.External {
		tools = append(tools, tool)
	}
	sort.Strings(tools)
	for _, tool := range tools {
		lines = append(lines, fmt.Sprintf("- **%s:** %s", tool, t.External[tool]))
	}
	if t.Origin.Workspace != "" {
		lines = append(lines, fmt.Sprintf("- **Planned in:** %s (`%s`)", t.Origin.Workspace, t.Origin.Path))
	}
	if planFolder != "" {
		lines = append(lines, fmt.Sprintf("- **Plan:** `%s/`", strings.TrimSuffix(planFolder, "/")))
	}
	if t.Spec != "" {
		lines = append(lines, fmt.Sprintf("- **Spec:** `%s`", t.Spec))
	}
	if len(t.ADRs) > 0 {
		lines = append(lines, "- **ADRs:** "+strings.Join(t.ADRs, ", "))
	}
	return strings.TrimRight(strings.Join(lines, "\n"), "\n") + "\n"
}

// PRArgs builds the `gh pr create` arguments, run in dir. With a ticket, the pull
// request links it and gets the title "[ID] Title" unless args gives one, and a
// branch without the ticket's id gets a warning.
func PRArgs(dir string, t *ticket.Ticket, planFolder string, args []string) []string {
	ghArgs := []string{"pr", "create"}
	title := ""
	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		title = args[0]
	}
	if t != nil {
		if title == "" {
			title = "[" + t.ID + "] " + t.Title
		}
		ghArgs = append(ghArgs, "--body", PRBody(t, planFolder))
		if branch, err := Output(dir, "git", "branch", "--show-current"); err == nil && !strings.Contains(strings.ToLower(branch), strings.ToLower(t.ID)) {
			fmt.Fprintf(os.Stderr, "warning: branch %s doesn't include %s — `daiku status` shows the suggested name\n", branch, t.ID)
		}
	}
	if title != "" {
		ghArgs = append(ghArgs, "--title", title)
	}
	return ghArgs
}
