import { PaneHeader } from "@/frame/PaneHeader";

interface Props {
  focused: boolean;
  onFill: () => void;
}

/** Placeholder: the markdown viewer arrives in S-0020. */
export function Viewer({ focused, onFill }: Props) {
  return (
    <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader title="Viewer" focused={focused} onFill={onFill} />
      <p className="p-4 text-list text-ink-faint">Arrives in S-0020 and S-0021.</p>
    </section>
  );
}
