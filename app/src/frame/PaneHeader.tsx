import { Maximize2, Minimize2 } from "lucide-react";
import { Button } from "@/components/ui/button";

interface Props {
  title: string;
  /** The pane fills the window, so the button restores the split. */
  focused: boolean;
  onFill: () => void;
}

/** 36px: the pane's title and its fill or restore button (D10). */
export function PaneHeader({ title, focused, onFill }: Props) {
  return (
    <div className="flex h-[36px] shrink-0 items-center justify-between border-b border-line px-3">
      <h2 className="text-label font-medium uppercase tracking-[var(--tracking-label)] text-ink-muted">
        {title}
      </h2>
      {focused ? (
        <Button variant="ghost" size="xs" onClick={onFill}>
          <Minimize2 />
          Restore
        </Button>
      ) : (
        <Button variant="ghost" size="icon-xs" onClick={onFill} aria-label={`Fill window with ${title}`}>
          <Maximize2 />
        </Button>
      )}
    </div>
  );
}
