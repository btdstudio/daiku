import { PaneHeader } from "@/frame/PaneHeader";
import { Badges } from "@/build/Badges";
import { useBuildBoard } from "@/build/useBuildBoard";
import type { TicketView } from "@/generated/TicketView";
import { Markdown } from "./Markdown";

interface Props {
  root: string | null;
  /** The ticket's `TicketView.path`: a key into the loaded tickets, nothing more. */
  ticketKey: string;
  focused: boolean;
  onFill: () => void;
  onClose?: () => void;
}

function fields(t: TicketView): [string, string][] {
  const rows: [string, string][] = [
    ["id", t.id],
    ["type", t.type],
    ["status", t.status],
    ["repo", t.repo],
    ["change", t.inbox || t.change === "" ? "Inbox" : t.changeClosed ? `${t.change} (closed)` : t.change],
    ["parent", t.parent],
    ["depends on", t.dependsOn.join(", ")],
    ["requirements", t.requirements.join(", ")],
    ["labels", t.labels.join(", ")],
    ["external", Object.entries(t.external).map(([k, v]) => `${k}=${v}`).join(", ")],
  ];
  return rows.filter(([, v]) => v !== "");
}

/** A ticket in the viewer (design D11): its fields, badges, build and body, from the
 *  loaded `TicketView`. It reads no file. */
export function TicketPanel({ root, ticketKey, focused, onFill, onClose }: Props) {
  const board = useBuildBoard(root);
  const t = board.tickets.find((x) => x.path === ticketKey);
  const info = t ? board.builds.get(t.id) : undefined;

  let body;
  if (t) {
    body = (
      <article className="p-4">
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-list">
          {fields(t).map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-ink-faint">{k}</dt>
              <dd className="min-w-0 break-words text-ink-secondary">{v}</dd>
            </div>
          ))}
        </dl>
        <div className="mt-2">
          <Badges ticket={t} />
        </div>
        {info && (
          <p className="mt-2 text-list text-ink-muted">
            Build: {info.build.phase} · {info.build.status}
            {info.holdsLock && <span> · holds the lock on {info.repo}</span>}
          </p>
        )}
        <div className="mt-3">
          <Markdown source={t.body} />
        </div>
      </article>
    );
  } else if (board.loading) {
    body = <p className="p-4 text-list text-ink-faint">Reading…</p>;
  } else if (board.error !== null && board.tickets.length === 0) {
    body = (
      <p role="alert" className="p-4 text-list text-ink-muted">
        {board.error}
      </p>
    );
  } else {
    body = <p className="p-4 text-list text-ink-faint">This ticket is no longer in the workspace</p>;
  }

  return (
    <section aria-label="Viewer" className="flex h-full min-w-0 flex-col bg-surface-window">
      <PaneHeader
        title={t ? `${t.id} · ${t.title}` : "Ticket"}
        focused={focused}
        onFill={onFill}
        onClose={onClose}
      />
      <div className="min-h-0 flex-1 overflow-auto">{body}</div>
    </section>
  );
}
