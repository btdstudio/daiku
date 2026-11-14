package state

import (
	"fmt"
	"path"
	"regexp"

	"github.com/btdstudio/daiku/internal/core/repos"
)

// The loop names daiku records in each change's Loop.
const (
	PlanLoopName  = "plan"
	BuildLoopName = "build"
)

// PlanDir holds plan changes, as plan/specs/<n>-<slug>/.
const PlanDir = "plan/specs"

// PlanLoop is the plan loop's layout around machine m: numbered changes named
// <n>-<slug>, each under plan/specs/.
func PlanLoop(m Machine) Loop {
	return Loop{Machine: m, Name: PlanNameRe, Dir: PlanDir}
}

// BuildLoop is the build loop's layout around machine m: each change is named by
// its ticket id, which name must match, and lives under
// build/<repo>/changes/<ticket-id>/, where <repo> is the change's first repo.
// The caller passes the ticket id grammar (ticket.IDRe) so that state stays
// independent of the ticket package.
func BuildLoop(m Machine, name *regexp.Regexp) Loop {
	return Loop{Machine: m, Name: name, Folder: buildFolder}
}

// buildFolder places a build change under its repo. The repo must be a valid
// logical name, so a record can't steer the folder out of build/.
func buildFolder(c Change) (string, error) {
	if len(c.Repos) == 0 {
		return "", fmt.Errorf("build change %q has no repo", c.Name)
	}
	repo := c.Repos[0]
	if !repos.NameRe.MatchString(repo) {
		return "", fmt.Errorf("build change %q has an invalid repo name %q", c.Name, repo)
	}
	return path.Join("build", repo, "changes", c.Name), nil
}
