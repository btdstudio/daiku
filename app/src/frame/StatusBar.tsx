import type { Fold } from "./fold";
import type { Tab } from "./viewState";

interface Props {
  fold: Fold;
  /** The selected workspace's `WorkspaceRef.name`, safe to show. */
  name: string | null;
  tab: Tab;
}

/** 30px. Left: the workspace and the tab; below 1440px the name only (D14).
 *  Right: the nudge slot, empty until S-0023. */
export function StatusBar({ fold, name, tab }: Props) {
  return (
    <footer className="flex h-[30px] shrink-0 items-center justify-between border-t border-line bg-surface-chrome px-3 text-label text-ink-muted">
      <span className="truncate">
        {name && <span className="font-mono text-ink-secondary">{name}</span>}
        {name && fold === "wide" && <span> · {tab === "plan" ? "Plan" : "Build"}</span>}
      </span>
      <span data-slot="nudge" />
    </footer>
  );
}
