// Package lock is the repo lock: only one build per repo runs implement through
// ship at a time. The lock isn't stored anywhere. It's worked out from STATE.json
// on each gate into implement, so closing or reopening the holder frees the repo
// with nothing to clean up (planning architecture D10).
package lock

import (
	"fmt"
	"sort"

	core "github.com/btdstudio/daiku/internal/core/state"
)

// held are the phases in which a build holds its repo.
var held = map[string]bool{"implement": true, "test": true, "review": true, "security": true, "ship": true}

// Holds reports whether a change holds its repo's lock: an open build in
// implement through ship that hasn't completed ship.
func Holds(c core.Change) bool {
	if c.Loop != core.BuildLoopName || c.IsClosed() || len(c.Repos) == 0 || !held[c.Phase] {
		return false
	}
	return !(c.Phase == "ship" && c.Status == core.Complete)
}

// Holder returns the first other build, by name, holding c's repo, and its
// record. It returns "" when the repo is free or c isn't a build.
func Holder(changes map[string]*core.Change, c core.Change) (string, core.Change) {
	if c.Loop != core.BuildLoopName || len(c.Repos) == 0 {
		return "", core.Change{}
	}
	names := make([]string, 0, len(changes))
	for name := range changes {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		other := changes[name]
		if other == nil || name == c.Name || !Holds(*other) || other.Repos[0] != c.Repos[0] {
			continue
		}
		return name, *other
	}
	return "", core.Change{}
}

// Message is the gate violation for a locked repo.
func Message(repo, holder string, at core.Change) string {
	return fmt.Sprintf("daiku: %s is locked by %s (%s %s) — finish, close or reopen it to tasks", repo, holder, at.Phase, at.Status)
}
