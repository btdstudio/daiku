package cli

import (
	"fmt"

	"github.com/btdstudio/daiku/internal/core/ticket"
	"github.com/btdstudio/daiku/internal/state"
)

// requirePlan refuses a build change for a verb that only works on plan changes,
// such as tickets ready or overview. The verbs every change has (status, switch,
// gate, reopen, change close, env) work on both loops.
func requirePlan(c state.State, verb string) error {
	if c.Loop == state.BuildLoop {
		return fmt.Errorf("daiku: %s is a build change — %s works on plan changes", c.Name, verb)
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

// changeTicket is a build's ticket, read in place from its source: the first
// source that parses as a ticket with an id. It's nil when there's none.
func changeTicket(root string, st state.State) *ticket.Ticket { return state.BuildTicket(root, st) }
