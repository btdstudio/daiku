import { BookOpen, Lightbulb, List, Scale, type LucideIcon } from "lucide-react";
import { type ReactNode, useState } from "react";
import { visibleRows } from "@/build/groups";
import { PaneHeader } from "@/frame/PaneHeader";
import type { Selected } from "@/viewer/selected";
import { usePlanTab } from "./usePlanTab";

interface Props {
  root: string | null;
  onFill: () => void;
  onSelect: (sel: Selected) => void;
}

type GroupName = "Plans" | "Ideas" | "ADRs";

const rowClass =
  "flex w-full flex-col gap-0.5 px-3 py-1.5 text-left outline-none hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring";

const headingClass =
  "flex items-center gap-2 px-3 pb-1 pt-3 text-label font-medium uppercase tracking-[var(--tracking-label)] text-ink-muted";

/** The Plan tab's list (design D8): Plans, Ideas, ADRs and a References heading,
 *  each with its icon and count; the first three fold at 8 rows. */
export function PlanList({ root, onFill, onSelect }: Props) {
  const tab = usePlanTab(root);
  // Expanded groups, local; a new workspace folds them again (reset during render).
  const [expanded, setExpanded] = useState<{ root: string | null; groups: GroupName[] }>({
    root,
    groups: [],
  });
  if (expanded.root !== root) setExpanded({ root, groups: [] });
  const isOpen = (g: GroupName) => expanded.root === root && expanded.groups.includes(g);

  function group<T>(
    name: GroupName,
    Icon: LucideIcon,
    items: T[],
    key: (i: T) => string,
    row: (i: T) => ReactNode,
  ) {
    const rows = visibleRows(items, isOpen(name));
    return (
      <div key={name} role="group" aria-label={name}>
        <h3 className={headingClass}>
          <Icon className="size-3.5" />
          {name}
          <span className="font-mono text-ink-faint">{items.length}</span>
        </h3>
        {items.length === 0 && !tab.loading && (
          <p className="px-3 py-1 text-list text-ink-faint">Nothing here yet</p>
        )}
        {rows.map((r) =>
          r.kind === "more" ? (
            <button
              key="more"
              type="button"
              onClick={() => setExpanded({ root, groups: [...expanded.groups, name] })}
              className="w-full px-3 py-1 text-left text-list text-ink-faint outline-none hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring"
            >
              {r.count} more
            </button>
          ) : (
            <div key={key(r.item)}>{row(r.item)}</div>
          ),
        )}
      </div>
    );
  }

  return (
    <section aria-label="Plan list" className="flex h-full min-w-0 flex-col bg-surface-chrome">
      <PaneHeader title="Plan" focused={false} onFill={onFill} />
      <div className="min-h-0 flex-1 overflow-auto">
        {tab.error !== null && (
          <p role="alert" className="px-3 py-2 text-list text-ink-muted">
            {tab.error}
          </p>
        )}
        {tab.loading && <p className="p-4 text-list text-ink-faint">Loading…</p>}
        {group(
          "Plans",
          List,
          tab.plans,
          (p) => p.name,
          (p) => (
            <button type="button" onClick={() => onSelect({ kind: "plan", name: p.name })} className={rowClass}>
              <span className="min-w-0 truncate text-list text-ink">{p.name}</span>
              <span className="text-label text-ink-faint">{p.phase || "intake"}</span>
            </button>
          ),
        )}
        {group(
          "Ideas",
          Lightbulb,
          tab.ideas,
          (i) => String(i.id),
          (i) => {
            const muted = i.status === "parked" || i.status === "dropped";
            return (
              <button type="button" onClick={() => onSelect({ kind: "idea", id: i.id })} className={rowClass}>
                <span
                  className={`flex items-baseline gap-2 text-list ${muted ? "text-ink-faint" : "text-ink"}`}
                >
                  <span className="font-mono text-id text-ink-secondary">{i.id}</span>
                  <span className="min-w-0 flex-1 truncate">{i.title}</span>
                </span>
                <span className="text-label text-ink-faint">{i.status}</span>
              </button>
            );
          },
        )}
        {group(
          "ADRs",
          Scale,
          tab.adrs,
          (a) => String(a.number),
          (a) => (
            <button type="button" onClick={() => onSelect({ kind: "adr", number: a.number })} className={rowClass}>
              <span className="flex items-baseline gap-2 text-list text-ink">
                <span className="font-mono text-id text-ink-secondary">
                  {String(a.number).padStart(4, "0")}
                </span>
                <span className="min-w-0 flex-1 truncate">{a.title}</span>
              </span>
              <span className="text-label text-ink-faint">{a.status}</span>
            </button>
          ),
        )}
        <h3 className={headingClass}>
          <BookOpen className="size-3.5" />
          References · later
        </h3>
      </div>
    </section>
  );
}
