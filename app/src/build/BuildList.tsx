import { Lock } from "lucide-react";
import { useState } from "react";
import { DRAFT_INK, NEXT_MARK } from "@/frame/marks";
import { PaneHeader } from "@/frame/PaneHeader";
import { Badges } from "./Badges";
import { COLUMN_LABEL, type ListColumn, visibleRows } from "./groups";
import { MARKS } from "./marks";
import { useBuildBoard } from "./useBuildBoard";

interface Props {
  root: string | null;
  onFill: () => void;
  /** A ticket was chosen: its `TicketView.path`, used only as a key. */
  onSelect: (path: string) => void;
  /** The ticket id to carry the next mark (S-0023). */
  nextId?: string | null;
}

/** The Build tab's list (design D7): four groups with status marks, folded at 8
 *  rows, from `useBuildBoard`. */
export function BuildList({ root, onFill, onSelect, nextId }: Props) {
  const board = useBuildBoard(root);
  // Expanded groups, local; a new workspace folds them again (reset during render).
  const [expanded, setExpanded] = useState<{ root: string | null; groups: ListColumn[] }>({
    root,
    groups: [],
  });
  if (expanded.root !== root) setExpanded({ root, groups: [] });
  const open = (c: ListColumn) => expanded.root === root && expanded.groups.includes(c);

  const empty = board.tickets.length === 0;
  return (
    <section aria-label="Build list" className="flex h-full min-w-0 flex-col bg-surface-chrome">
      <PaneHeader title="Build" focused={false} onFill={onFill} />
      <div className="min-h-0 flex-1 overflow-auto">
        {board.error !== null && (
          <p role="alert" className="px-3 py-2 text-list text-ink-muted">
            {board.error}
          </p>
        )}
        {board.unreadable.length > 0 && (
          <details className="px-3 py-2 text-list text-ink-muted">
            <summary className="cursor-pointer">{board.unreadable.length} unreadable</summary>
            <ul className="mt-1 flex flex-col gap-1">
              {board.unreadable.map((p, i) => (
                <li key={`${p.id}:${i}`}>
                  <span className="font-mono text-id">{p.id}</span> {p.message}
                </li>
              ))}
            </ul>
          </details>
        )}
        {board.loading && empty && <p className="p-4 text-list text-ink-faint">Loading…</p>}
        {!board.loading && empty && board.error === null && (
          <p className="p-4 text-list text-ink-faint">No tickets yet</p>
        )}
        {!empty &&
          board.groups.map((g) => {
            const Mark = MARKS[g.column];
            const rows = visibleRows(g.tickets, open(g.column));
            return (
              <div key={g.column} role="group" aria-label={COLUMN_LABEL[g.column]}>
                <h3
                  className={`flex items-center gap-2 px-3 pb-1 pt-3 text-label font-medium uppercase tracking-[var(--tracking-label)] ${
                    g.column === "draft" ? DRAFT_INK : "text-ink-muted"
                  }`}
                >
                  <Mark className="size-3.5" />
                  {COLUMN_LABEL[g.column]}
                  <span className="font-mono text-ink-faint">{g.tickets.length}</span>
                </h3>
                {rows.map((r) => {
                  if (r.kind === "more") {
                    return (
                      <button
                        key="more"
                        type="button"
                        onClick={() =>
                          setExpanded({ root, groups: [...expanded.groups, g.column] })
                        }
                        className="w-full px-3 py-1 text-left text-list text-ink-faint outline-none hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring"
                      >
                        {r.count} more
                      </button>
                    );
                  }
                  const t = r.item;
                  const info = g.column === "inBuild" ? board.builds.get(t.id) : undefined;
                  return (
                    <button
                      key={t.path}
                      type="button"
                      onClick={() => onSelect(t.path)}
                      className={`flex w-full flex-col gap-0.5 px-3 py-1.5 text-left outline-none hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring ${
                        t.id === nextId ? NEXT_MARK : ""
                      }`}
                    >
                      <span className="flex items-baseline gap-2 text-list text-ink">
                        <span className="font-mono text-id text-ink-secondary">{t.id}</span>
                        <span className="min-w-0 flex-1 truncate">{t.title}</span>
                      </span>
                      <span className="flex flex-wrap items-center gap-2 text-label text-ink-faint">
                        {t.repo !== "" && <span>{t.repo}</span>}
                        <Badges ticket={t} />
                        {info && <span>{info.build.phase}</span>}
                        {info?.holdsLock && (
                          <span className="flex items-center gap-0.5" title="Holds the repo lock">
                            <Lock className="size-3" aria-label="repo lock" />
                            {info.repo}
                          </span>
                        )}
                      </span>
                    </button>
                  );
                })}
              </div>
            );
          })}
      </div>
    </section>
  );
}
