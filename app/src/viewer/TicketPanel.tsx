import { PaneHeader } from "@/frame/PaneHeader";
import { Badges } from "@/build/Badges";
import { StatusText } from "@/build/StatusText";
import { useBuildBoard } from "@/build/useBuildBoard";
import { useReadTicket } from "@/data/hooks";
import { QueryFeedback } from "@/data/QueryFeedback";
import type { ReactNode } from "react";
import type { TicketView } from "@/generated/TicketView";
import { Markdown } from "./Markdown";
import { splitFrontMatter } from "./splitFrontMatter";

/** Shared, so a view without warnings doesn't allocate one each render. */
const EMPTY_WARNINGS: string[] = [];

interface Props {
  root: string | null;
  /** The ticket's `TicketView.path`: a key into the loaded tickets, nothing more. */
  ticketKey: string;
  focused: boolean;
  onFill: () => void;
  onClose?: () => void;
}

function fields(t: TicketView): [string, ReactNode][] {
  const rows: [string, ReactNode][] = [
    ["id", t.id],
    ["type", t.type],
    ["status", t.status === "" ? "" : <StatusText status={t.status} />],
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

/** A ticket in the viewer (design D11): its fields, badges and build from the loaded
 *  `TicketView`, and its body read through `read_ticket` (`S-0019` D7). The list's body
 *  holds until the read resolves; a failed read keeps the last good text and metadata. */
export function TicketPanel({ root, ticketKey, focused, onFill, onClose }: Props) {
  const board = useBuildBoard(root);
  const t = board.tickets.find((x) => x.path === ticketKey);
  const info = t ? board.builds.get(t.id) : undefined;
  const read = useReadTicket(root, t?.path ?? null);

  const warnings =
    board.warnings.length > 0 || read.warnings.length > 0
      ? [...board.warnings, ...read.warnings]
      : EMPTY_WARNINGS;
  const error = board.error ?? read.error;

  let body;
  if (t) {
    // The list body is only the initial/fallback source; a successful read supplies
    // the current raw text. Its front matter would duplicate the fields above.
    const source = splitFrontMatter(read.data ?? t.body).body;
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
          <Markdown source={source} />
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
      <div className="min-h-0 flex-1 overflow-auto">
        {t && <QueryFeedback warnings={warnings} error={error} />}
        {body}
      </div>
    </section>
  );
}
