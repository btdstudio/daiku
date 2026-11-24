import { PaneHeader } from "@/frame/PaneHeader";
import { AdrPanel } from "@/plan/AdrPanel";
import { IdeaPanel } from "@/plan/IdeaPanel";
import { PlanPanel } from "@/plan/PlanPanel";
import type { Selected } from "./selected";
import { TicketPanel } from "./TicketPanel";

interface Props {
  root: string | null;
  /** The open ticket, plan, idea or ADR, or null when nothing is open. */
  selected: Selected | null;
  focused: boolean;
  onFill: () => void;
  /** Close what's open, back to the list. */
  onClose?: () => void;
}

/** Shows the selection, read-only, each kind in its own panel. */
export function Viewer({ root, selected, focused, onFill, onClose }: Props) {
  const common = { root, focused, onFill, onClose };
  switch (selected?.kind) {
    case "ticket":
      return <TicketPanel {...common} ticketKey={selected.key} />;
    case "plan":
      return (
        <PlanPanel
          {...common}
          name={selected.name}
          initialPhase={selected.phase}
          nonce={selected.nonce}
        />
      );
    case "idea":
      return <IdeaPanel {...common} id={selected.id} />;
    case "adr":
      return <AdrPanel {...common} number={selected.number} />;
    default:
      return (
        <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
          <PaneHeader title="Viewer" focused={focused} onFill={onFill} />
          <div className="min-h-0 flex-1 overflow-auto">
            <p className="p-4 text-list text-ink-faint">Open a file from the list</p>
          </div>
        </section>
      );
  }
}
