import { PaneHeader } from "@/frame/PaneHeader";

/** Placeholder: the Plan list arrives in S-0022. */
export function PlanList({ onFill }: { onFill: () => void }) {
  return (
    <section aria-label="Plan list" className="flex h-full min-w-0 flex-col bg-surface-chrome">
      <PaneHeader title="Plan" focused={false} onFill={onFill} />
      <p className="p-4 text-list text-ink-faint">Arrives in S-0022.</p>
    </section>
  );
}
