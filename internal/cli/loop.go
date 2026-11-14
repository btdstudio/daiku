package cli

import (
	"fmt"

	"github.com/btdstudio/daiku/internal/state"
)

// requirePlan refuses a change whose loop has no commands yet. Every verb that
// acts on a change goes through it, so the version that brings build commands
// deletes this one function.
func requirePlan(c state.State) error {
	if c.Loop == state.BuildLoop {
		return fmt.Errorf("daiku: %s is a build change — build commands aren't in this version yet", c.Name)
	}
	return nil
}

// recorded returns the named change's record, or a zero State when there's none.
func recorded(root, name string) (state.State, error) {
	changes, err := state.ReadChanges(root)
	if err != nil {
		return state.State{}, err
	}
	if c := changes[name]; c != nil {
		return *c, nil
	}
	return state.State{}, nil
}
