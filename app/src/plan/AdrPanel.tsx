import { failureText } from "@/data/envelope";
import { useAdrs } from "@/data/hooks";
import { QueryFeedback } from "@/data/QueryFeedback";
import { PaneHeader } from "@/frame/PaneHeader";
import { FrontMatter } from "@/viewer/FrontMatter";
import { Markdown } from "@/viewer/Markdown";
import { splitFrontMatter } from "@/viewer/splitFrontMatter";

interface Props {
  root: string | null;
  number: number;
  focused: boolean;
  onFill: () => void;
  onClose?: () => void;
}

/** An ADR in the viewer (design D7): its front matter as a mono block, then its body
 *  as markdown, from the cached ADR list. */
export function AdrPanel({ root, number, focused, onFill, onClose }: Props) {
  const adrs = useAdrs(root);
  const adr = adrs.data?.find((a) => a.number === number);

  let body;
  if (adr) {
    const { front, body: md } = splitFrontMatter(adr.raw);
    body = (
      <article className="p-4">
        {front !== null && <FrontMatter text={front} />}
        <Markdown source={md} />
      </article>
    );
  } else if (adrs.isPending) {
    body = <p className="p-4 text-list text-ink-faint">Reading…</p>;
  } else if (adrs.error && !adrs.data) {
    body = null;
  } else {
    body = <p className="p-4 text-list text-ink-faint">This ADR is no longer in the workspace</p>;
  }

  return (
    <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader
        title={adr ? `ADR-${String(adr.number).padStart(4, "0")} ${adr.title}` : "ADR"}
        focused={focused}
        onFill={onFill}
        onClose={onClose}
      />
      <div className="min-h-0 flex-1 overflow-auto">
        {adrs.error && (
          <p role="alert" className="px-4 py-2 text-list text-ink-muted">
            {failureText(adrs.error)}
          </p>
        )}
        <QueryFeedback warnings={adrs.warnings} />
        {body}
      </div>
    </section>
  );
}
