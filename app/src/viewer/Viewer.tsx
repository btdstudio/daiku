import { PaneHeader } from "@/frame/PaneHeader";
import { DevPicker } from "./DevPicker";
import { FrontMatter } from "./FrontMatter";
import { Markdown } from "./Markdown";
import { splitFrontMatter } from "./splitFrontMatter";
import { useReadText } from "./useReadText";

interface Props {
  root: string | null;
  /** A workspace-relative path, or null when no file is open. */
  path: string | null;
  focused: boolean;
  onFill: () => void;
  /** Dev only: choose a file from the picker. */
  onPick?: (path: string) => void;
  /** Close the open file, back to the list (and, in dev, the picker). */
  onClose?: () => void;
}

/** "File too large to show (N MiB)" for the `TooLarge` Failure, from its message. */
function tooLarge(message: string): string {
  const n = /\(([\d.]+) MiB\)/.exec(message)?.[1];
  return n ? `File too large to show (${n} MiB)` : "File too large to show";
}

/** Shows one workspace file, read-only: front matter as a mono block, then the
 *  markdown body. */
export function Viewer({ root, path, focused, onFill, onPick, onClose }: Props) {
  const read = useReadText(root, path);
  let body;
  if (read.state === "idle") {
    body = (
      <>
        <p className="p-4 text-list text-ink-faint">Open a file from the list</p>
        {import.meta.env.DEV && root !== null && onPick && (
          <DevPicker root={root} onPick={onPick} />
        )}
      </>
    );
  } else if (read.state === "loading") {
    body = <p className="p-4 text-list text-ink-faint">Reading…</p>;
  } else if (read.state === "failed") {
    const { kind, message } = read.failure.error;
    body = (
      <p role="alert" className="p-4 text-list text-ink-muted">
        {kind === "TooLarge" ? tooLarge(message) : message}
      </p>
    );
  } else {
    const { front, body: md } = splitFrontMatter(read.text);
    body = (
      <article className="p-4">
        {front !== null && <FrontMatter text={front} />}
        <Markdown source={md} />
      </article>
    );
  }
  return (
    <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader
        title={path ?? "Viewer"}
        focused={focused}
        onFill={onFill}
        onClose={path !== null ? onClose : undefined}
      />
      <div className="min-h-0 flex-1 overflow-auto">{body}</div>
    </section>
  );
}
