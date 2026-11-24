import { PaneHeader } from "@/frame/PaneHeader";

/** Placeholder: the Build board arrives in S-0021, the Plan board in S-0022. The
 *  board is only shown as a focus, so its button always restores the split. */
export function Board({ onRestore }: { onRestore: () => void }) {
  return (
    <section aria-label="Board" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader title="Board" focused onFill={onRestore} />
      <p className="p-4 text-list text-ink-faint">Arrives in S-0020 and S-0022.</p>
    </section>
  );
}
