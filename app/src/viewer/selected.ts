/** What the viewer shows: a ticket by its `TicketView.path`, which is only a key into
 *  the loaded tickets and is never passed to a command; a plan by name (and, from the
 *  nudge, a phase to start on, with a click count so a repeat click starts again); an
 *  idea by id; an ADR by number. Frame state, not view state (S-0021 R11). */
export type Selected =
  | { kind: "ticket"; key: string }
  | { kind: "plan"; name: string; phase?: string; nonce?: number }
  | { kind: "idea"; id: number }
  | { kind: "adr"; number: number };
