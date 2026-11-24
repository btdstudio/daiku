/** What the viewer shows: a ticket by its `TicketView.path`, which is only a key into
 *  the loaded tickets and is never passed to a command; a plan by name; an idea by id;
 *  an ADR by number. Frame state, not view state (S-0021 R11). */
export type Selected =
  | { kind: "ticket"; key: string }
  | { kind: "plan"; name: string }
  | { kind: "idea"; id: number }
  | { kind: "adr"; number: number };
