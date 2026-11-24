import { PaneHeader } from "@/frame/PaneHeader";
import type { Tab } from "@/frame/viewState";
import type { Selected } from "@/viewer/selected";
import { BuildBoard } from "./BuildBoard";
import { PlanBoard } from "./PlanBoard";

interface Props {
  tab: Tab;
  root: string | null;
  /** A card was chosen: a ticket (by its `TicketView.path`) or a plan (by name). */
  onSelect: (sel: Selected) => void;
  onRestore: () => void;
  /** The marked ticket id (Build) and plan name (Plan), for the next marks. */
  nextTicket?: string | null;
  nextPlan?: string | null;
}

/** The board, shown only as a focus, so its button always restores the split. The
 *  tab picks the board. Choosing a card selects it and leaves Board focus for the
 *  split, where the viewer is. */
export function Board({ tab, root, onSelect, onRestore, nextTicket, nextPlan }: Props) {
  return (
    <section aria-label="Board" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader title="Board" focused onFill={onRestore} />
      {tab === "build" ? (
        <BuildBoard
          root={root}
          nextId={nextTicket}
          onSelect={(key) => {
            onSelect({ kind: "ticket", key });
            onRestore();
          }}
        />
      ) : (
        <PlanBoard
          root={root}
          nextPlan={nextPlan}
          onSelect={(name) => {
            onSelect({ kind: "plan", name });
            onRestore();
          }}
        />
      )}
    </section>
  );
}
