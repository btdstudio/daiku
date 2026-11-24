import { Plus, X } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { chipLabels } from "./chips";
import { pickAndOpen } from "./openWorkspace";
import type { Action, WorkspaceRef } from "./viewState";

interface Props {
  /** The open workspaces, in chip order. */
  open: WorkspaceRef[];
  selected: string | null;
  dispatch: (a: Action) => void;
}

/** 52px: one chip per open workspace, and "+" (R1, R2, R7). */
export function WorkspacesRail({ open, selected, dispatch }: Props) {
  const labels = chipLabels(open.map((w) => w.name));
  return (
    <nav
      aria-label="Workspaces"
      className="flex w-[52px] shrink-0 flex-col items-center gap-2 border-r border-line bg-surface-chrome py-3"
    >
      {open.map((w, i) => (
        <div key={w.root} className="group/chip relative">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label={w.name}
                aria-current={w.root === selected ? "true" : undefined}
                onClick={() => dispatch({ type: "selected", root: w.root })}
                className={`flex size-9 items-center justify-center rounded-md font-mono text-label font-semibold outline-none focus-visible:ring-2 focus-visible:ring-ring ${
                  w.root === selected
                    ? "bg-surface-selected text-ink"
                    : "bg-surface-raised text-ink-muted hover:text-ink"
                }`}
              >
                {labels[i]}
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <span className="font-medium">{w.name}</span>
              <span className="text-ink-faint">{w.shown}</span>
            </TooltipContent>
          </Tooltip>
          <button
            type="button"
            aria-label={`Close ${w.name}`}
            onClick={() => dispatch({ type: "closed", root: w.root })}
            className="absolute -top-1 -right-1 hidden size-4 items-center justify-center rounded-full border border-line bg-surface-window text-ink-muted outline-none group-focus-within/chip:flex group-hover/chip:flex hover:text-ink"
          >
            <X className="size-3" />
          </button>
        </div>
      ))}
      <div className="mt-auto">
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              type="button"
              aria-label="Open workspace"
              onClick={() => void pickAndOpen(dispatch)}
              className="flex size-9 items-center justify-center rounded-md border border-dashed border-line text-ink-muted outline-none hover:text-ink focus-visible:ring-2 focus-visible:ring-ring"
            >
              <Plus className="size-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="right">Open workspace…</TooltipContent>
        </Tooltip>
      </div>
    </nav>
  );
}
