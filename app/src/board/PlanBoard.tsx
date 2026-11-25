import { QueryFeedback } from "@/data/QueryFeedback";
import { NEXT_MARK } from "@/frame/marks";
import { useStatus } from "@/data/hooks";
import type { Change } from "@/generated/Change";
import { NOT_STARTED } from "@/plan/phaseStrip";
import { planColumn } from "@/plan/board";

interface Props {
  root: string | null;
  /** A card was chosen: the plan's name. */
  onSelect: (name: string) => void;
  /** The plan's name to carry the next mark (S-0023). */
  nextPlan?: string | null;
}

const NONE: never[] = [];

/** The Plan board (design D9): one column per plan phase, each open plan one card in
 *  the column of its current phase (the first when it has none). */
export function PlanBoard({ root, onSelect, nextPlan }: Props) {
  const status = useStatus(root);
  const phases = status.data?.planPhases ?? NONE;
  const plans = status.data?.plans ?? NONE;

  const byColumn = new Map<string, Change[]>(phases.map((p) => [p, []]));
  for (const p of [...plans].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
    byColumn.get(planColumn(p, phases))?.push(p);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <QueryFeedback warnings={status.warnings} error={status.error} />
      {status.isPending && root !== null && <p className="p-4 text-list text-ink-faint">Loading…</p>}
      {status.data && plans.length === 0 && (
        <p className="px-3 pt-3 text-list text-ink-faint">No open plans</p>
      )}
      <div
        className="grid min-h-0 flex-1 gap-3 overflow-auto p-3"
        style={{ gridTemplateColumns: `repeat(${phases.length || 1}, minmax(160px, 1fr))` }}
      >
        {phases.map((phase) => {
          const cards = byColumn.get(phase) ?? [];
          return (
            <section key={phase} role="group" aria-label={phase} className="flex min-w-0 flex-col gap-2">
              <h3 className="text-label font-medium uppercase tracking-[var(--tracking-label)] text-ink-muted">
                {phase} <span className="font-mono text-ink-faint">{cards.length}</span>
              </h3>
              {cards.map((p) => (
                <button
                  key={p.name}
                  type="button"
                  onClick={() => onSelect(p.name)}
                  className={`flex flex-col gap-1 rounded-md border border-line p-2 text-left outline-none hover:bg-surface-selected focus-visible:ring-2 focus-visible:ring-ring ${
                    p.name === nextPlan ? NEXT_MARK : "bg-surface-raised"
                  }`}
                >
                  <span className="min-w-0 break-words text-list text-ink">{p.name}</span>
                  {p.repos.length > 0 && (
                    <span className="text-label text-ink-faint">{p.repos.join(", ")}</span>
                  )}
                  <span className="text-label text-ink-muted">{p.gates[phase] ?? NOT_STARTED}</span>
                  <span className="font-mono text-label text-ink-faint">{p.updatedAt}</span>
                </button>
              ))}
            </section>
          );
        })}
      </div>
    </div>
  );
}
