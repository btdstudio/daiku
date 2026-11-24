import { PaneHeader } from "@/frame/PaneHeader";

/** Placeholder: the Build ticket list arrives in S-0021. */
export function BuildList({ onFill }: { onFill: () => void }) {
  return (
    <section aria-label="Build list" className="flex h-full min-w-0 flex-col bg-surface-chrome">
      <PaneHeader title="Build" focused={false} onFill={onFill} />
      <p className="p-4 text-list text-ink-faint">Arrives in S-0020.</p>
    </section>
  );
}
