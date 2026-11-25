import { failureText } from "@/data/envelope";
import { useIdeas } from "@/data/hooks";
import { QueryFeedback } from "@/data/QueryFeedback";
import { PaneHeader } from "@/frame/PaneHeader";
import { Markdown } from "@/viewer/Markdown";

interface Props {
  root: string | null;
  id: number;
  focused: boolean;
  onFill: () => void;
  onClose?: () => void;
}

/** An idea in the viewer (design D7): a small `<dl>` of its fields, then its body as
 *  markdown, from the cached idea list. */
export function IdeaPanel({ root, id, focused, onFill, onClose }: Props) {
  const ideas = useIdeas(root);
  const idea = ideas.data?.find((i) => i.id === id);

  let body;
  if (idea) {
    const rows: [string, string][] = [
      ["status", idea.status],
      ["reason", idea.reason],
      ["updated", idea.updated],
      ["promoted", idea.changes.join(", ")],
    ];
    body = (
      <article className="p-4">
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-list">
          {rows
            .filter(([, v]) => v !== "")
            .map(([k, v]) => (
              <div key={k} className="contents">
                <dt className="text-ink-faint">{k}</dt>
                <dd className="min-w-0 break-words text-ink-secondary">{v}</dd>
              </div>
            ))}
        </dl>
        <div className="mt-3">
          <Markdown source={idea.body} />
        </div>
      </article>
    );
  } else if (ideas.isPending) {
    body = <p className="p-4 text-list text-ink-faint">Reading…</p>;
  } else if (ideas.error && !ideas.data) {
    body = null;
  } else {
    body = <p className="p-4 text-list text-ink-faint">This idea is no longer in the workspace</p>;
  }

  return (
    <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader title={idea ? idea.title : "Idea"} focused={focused} onFill={onFill} onClose={onClose} />
      <div className="min-h-0 flex-1 overflow-auto">
        {ideas.error && (
          <p role="alert" className="px-4 py-2 text-list text-ink-muted">
            {failureText(ideas.error)}
          </p>
        )}
        <QueryFeedback warnings={ideas.warnings} />
        {body}
      </div>
    </section>
  );
}
