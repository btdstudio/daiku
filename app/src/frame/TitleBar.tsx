import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { Focus } from "./fold";
import type { Tab } from "./viewState";

interface Props {
  tab: Tab;
  onTab: (tab: Tab) => void;
  focus: Focus;
  onFocus: (focus: Focus) => void;
}

const FOCUSES: { value: Focus; label: string }[] = [
  { value: "split", label: "Split" },
  { value: "viewer", label: "Viewer" },
  { value: "board", label: "Board" },
];

/** 44px, and it is the window's title bar: the native one is hidden (`titleBarStyle:
 *  Overlay`) and the traffic lights sit in the left padding. The Plan/Build switch
 *  sits in the middle of a 1fr auto 1fr grid, so it is centred in the window at
 *  every width (R8, R14). */
export function TitleBar({ tab, onTab, focus, onFocus }: Props) {
  return (
    <header
      data-tauri-drag-region
      className="grid h-[44px] shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-line bg-surface-chrome pr-3 pl-[88px]"
    >
      <span data-tauri-drag-region className="text-body font-semibold text-ink">
        daiku
      </span>
      <ToggleGroup
        type="single"
        size="sm"
        spacing={0}
        value={tab}
        onValueChange={(v) => v && onTab(v as Tab)}
        aria-label="Plan or Build"
      >
        <ToggleGroupItem value="plan" className="text-list">
          Plan
        </ToggleGroupItem>
        <ToggleGroupItem value="build" className="text-list">
          Build
        </ToggleGroupItem>
      </ToggleGroup>
      <ToggleGroup
        type="single"
        size="sm"
        spacing={0}
        value={focus}
        onValueChange={(v) => v && onFocus(v as Focus)}
        aria-label="Focus"
        className="justify-self-end"
      >
        {FOCUSES.map((f) => (
          <ToggleGroupItem key={f.value} value={f.value} className="text-list">
            {f.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </header>
  );
}
