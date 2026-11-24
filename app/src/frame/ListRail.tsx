import {
  BookOpen,
  CircleCheck,
  CircleDashed,
  Circle,
  Contrast,
  Lightbulb,
  List,
  Scale,
  type LucideIcon,
} from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import type { Tab } from "./viewState";

const GROUPS: Record<Tab, { label: string; icon: LucideIcon }[]> = {
  build: [
    { label: "In build", icon: Contrast },
    { label: "Ready", icon: Circle },
    { label: "Draft", icon: CircleDashed },
    { label: "Done", icon: CircleCheck },
  ],
  plan: [
    { label: "Plans", icon: List },
    { label: "Ideas", icon: Lightbulb },
    { label: "ADRs", icon: Scale },
    { label: "References", icon: BookOpen },
  ],
};

interface Props {
  tab: Tab;
  /** The tab's list, shown in the overlay. */
  children: ReactNode;
}

/** The folded list (D9, D15): 52px, each group's icon with a count (0 until lists
 *  have data). Clicking opens the list as a 340px overlay over the viewer's right
 *  edge; Escape or a click outside closes it. Local state, not view state. */
export function ListRail({ tab, children }: Props) {
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
        {GROUPS[tab].map(({ label, icon: Icon }) => (
          <span key={label} title={label} className="flex flex-col items-center gap-0.5 text-ink-faint">
            <Icon className="size-4" />
            <span className="font-mono text-label">0</span>
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
