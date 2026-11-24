import { BookOpen, Lightbulb, List, Scale, type LucideIcon } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { COLUMN_LABEL, LIST_ORDER } from "@/build/groups";
import { MARKS } from "@/build/marks";
import type { Tab } from "./viewState";

const GROUPS: Record<Tab, { key: string; label: string; icon: LucideIcon }[]> = {
  build: LIST_ORDER.map((c) => ({ key: c, label: COLUMN_LABEL[c], icon: MARKS[c] })),
  plan: [
    { key: "Plans", label: "Plans", icon: List },
    { key: "Ideas", label: "Ideas", icon: Lightbulb },
    { key: "ADRs", label: "ADRs", icon: Scale },
    { key: "References", label: "References", icon: BookOpen },
  ],
};

interface Props {
  tab: Tab;
  /** Each group's whole size, keyed by group name; a missing count shows 0. */
  counts?: Record<string, number>;
  /** The group whose count is amber: the one holding the next mark (S-0023 D9). */
  nextGroup?: string;
  /** The tab's list, shown in the overlay. */
  children: ReactNode;
}

/** The folded list (D9, D15): 52px, each group's icon with a count (Build's and
 *  Plan's from their lists). Clicking opens the list as a 340px overlay over the viewer's
 *  right edge; Escape or a click outside closes it. Local state, not view state. */
export function ListRail({ tab, counts, nextGroup, children }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    const onDown = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [open]);

  return (
    <div ref={root} className="relative h-full">
      <button
        type="button"
        aria-label={open ? "Close list" : "Open list"}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex h-full w-[52px] flex-col items-center gap-3 border-l border-line bg-surface-chrome py-3 outline-none hover:bg-surface-raised focus-visible:ring-2 focus-visible:ring-ring"
      >
        {GROUPS[tab].map(({ key, label, icon: Icon }) => (
          <span
            key={key}
            title={label}
            className="flex flex-col items-center gap-0.5 text-ink-faint"
          >
            <Icon className="size-4" />
            <span
              className={`font-mono text-label ${key === nextGroup ? "text-attention-ink" : ""}`}
            >
              {counts?.[key] ?? 0}
            </span>
          </span>
        ))}
      </button>
      {open && (
        <div className="absolute inset-y-0 right-[52px] z-10 w-[340px] border-l border-line bg-surface-chrome">
          {children}
        </div>
      )}
    </div>
  );
}
