import { PaneHeader } from "@/frame/PaneHeader";
import type { Tab } from "@/frame/viewState";
import { BuildBoard } from "./BuildBoard";

interface Props {
  tab: Tab;
  root: string | null;
  /** A ticket card was chosen: its `TicketView.path`. */
  onSelect: (path: string) => void;
  onRestore: () => void;
}

/** The board, shown only as a focus, so its button always restores the split. The
 *  Build tab shows the Build board; the Plan board arrives in S-0022. Choosing a card
 *  selects its ticket and leaves Board focus for the split, where the viewer is. */
export function Board({ tab, root, onSelect, onRestore }: Props) {
  return (
    <section aria-label="Board" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader title="Board" focused onFill={onRestore} />
      {tab === "build" ? (
        <BuildBoard
          root={root}
          onSelect={(path) => {
            onSelect(path);
            onRestore();
          }}
        />
      ) : (
        <p className="p-4 text-list text-ink-faint">Arrives in S-0022.</p>
      )}
    </section>
  );
}
