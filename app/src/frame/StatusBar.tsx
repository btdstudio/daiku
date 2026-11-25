import { QueryFeedback } from "@/data/QueryFeedback";
import type { Fold } from "./fold";
import type { Tab } from "./viewState";

interface Props {
  fold: Fold;
  /** The selected workspace's `WorkspaceRef.name`, safe to show. */
  name: string | null;
  tab: Tab;
  /** The active change's name and phase, as the envelope delivers them. */
  active?: { name: string; phase: string } | null;
  /** Core's warnings, status first then the nudge's queries (`S-0019`), as text. */
  warnings?: string[];
  /** The latest `next` read failure, shown beside the nudge as text. */
  error?: unknown;
  /** The nudge's text, or null for none. Rendered as text, never HTML. */
  nudge: string | null;
  /** Makes the nudge a button; without it the text is shown but not clickable. */
  onNudge?: () => void;
}

const NUDGE = "min-w-0 max-w-[60%] truncate whitespace-nowrap text-attention-ink";

/** 30px. Left: the workspace, the tab (wide only, D14) and the active change with its
 *  phase. Right: the queries' warnings/error (S-0019) and the nudge (S-0023). */
export function StatusBar({ fold, name, tab, active, warnings = [], error, nudge, onNudge }: Props) {
  return (
    <footer className="flex h-[30px] shrink-0 items-center justify-between gap-3 border-t border-line bg-surface-chrome px-3 text-label text-ink-muted">
      <span className="min-w-0 truncate">
        {name && <span className="font-mono text-ink-secondary">{name}</span>}
        {name && fold === "wide" && <span> · {tab === "plan" ? "Plan" : "Build"}</span>}
        {name && active && (
          <span data-slot="active">
            {" · "}
            <span className="font-mono text-ink-secondary">{active.name}</span>{" "}
            <span className="text-ink-muted">{active.phase}</span>
          </span>
        )}
      </span>
      {(warnings.length > 0 || error != null || nudge !== null) && (
        <span className="flex min-w-0 items-center justify-end gap-3">
          <QueryFeedback inline warnings={warnings} error={error} />
          {nudge !== null &&
            (onNudge ? (
              <button
                type="button"
                data-slot="nudge"
                title={nudge}
                onClick={onNudge}
                className={`${NUDGE} cursor-pointer text-right outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring`}
              >
                {nudge}
              </button>
            ) : (
              <span data-slot="nudge" title={nudge} className={NUDGE}>
                {nudge}
              </span>
            ))}
        </span>
      )}
    </footer>
  );
}
