import type { TicketView } from "@/generated/TicketView";

function Pill({ children }: { children: string }) {
  return (
    <span className="rounded-pill bg-surface-raised px-1.5 py-px text-label text-ink-muted">
      {children}
    </span>
  );
}

/** The drifted, published and frozen badges of a ticket: neutral pills (no amber;
 *  S-0023 owns that). */
export function Badges({ ticket }: { ticket: TicketView }) {
  if (!ticket.drifted && ticket.status !== "published" && !ticket.frozen) return null;
  return (
    <span className="flex flex-wrap gap-1">
      {ticket.drifted && <Pill>drifted</Pill>}
      {ticket.status === "published" && <Pill>published</Pill>}
      {ticket.frozen && <Pill>frozen</Pill>}
    </span>
  );
}
