// Package layout holds the build room's paths, relative to the workspace root, so
// no build package hard-codes them.
package layout

const (
	// Room is the build room's folder.
	Room = "build"
	// TicketsDir is the inbox: every unplanned ticket, native (N-) or pulled (X-).
	TicketsDir = "build/tickets"
)
