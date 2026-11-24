/** What the viewer shows: a workspace file (the dev picker), or a ticket by its
 *  `TicketView.path`, which is only a key into the loaded tickets and is never passed
 *  to a command. Frame state, not view state (S-0021 R11). */
export type Selected = { kind: "file"; path: string } | { kind: "ticket"; key: string };
