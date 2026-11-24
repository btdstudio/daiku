import { Button } from "@/components/ui/button";
import { openRoot, pickAndOpen } from "./openWorkspace";
import type { Action, WorkspaceRef } from "./viewState";

interface Props {
  /** The recents, most recent first. */
  recents: WorkspaceRef[];
  dispatch: (a: Action) => void;
}

/** Shown when no workspace is open (R6). No drop zone. */
export function LaunchScreen({ recents, dispatch }: Props) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-6 p-6">
      <h1 className="text-title font-semibold">daiku</h1>
      <Button onClick={() => void pickAndOpen(dispatch)}>Open workspace…</Button>
      {recents.length > 0 && (
        <section aria-label="Recent workspaces" className="flex w-full max-w-md flex-col gap-1">
          <h2 className="text-label uppercase tracking-[var(--tracking-label)] text-ink-muted">
            Recent
          </h2>
          {recents.map((w) => (
            <button
              key={w.root}
              type="button"
              onClick={() => void openRoot(w.root, dispatch)}
              className="flex flex-col items-start rounded-md px-3 py-2 text-left outline-none hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring"
            >
              <span className="text-body text-ink">{w.name}</span>
              <span className="w-full truncate text-label text-ink-faint">{w.shown}</span>
            </button>
          ))}
        </section>
      )}
    </div>
  );
}
