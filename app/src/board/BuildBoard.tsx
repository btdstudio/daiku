import { useState } from "react";
import { Badges } from "@/build/Badges";
import {
  BOARD_ORDER,
  COLUMN_LABEL,
  NO_REPO,
  type PlanFilter,
  type RepoFilter,
  filterTickets,
  placeAll,
  planOptions,
  repoOptions,
} from "@/build/groups";
import { useBuildBoard } from "@/build/useBuildBoard";
import { QueryFeedback } from "@/data/QueryFeedback";
import { DRAFT_INK, NEXT_MARK } from "@/frame/marks";
import type { BuildInfo } from "./columns";
import type { TicketView } from "@/generated/TicketView";

interface Props {
  root: string | null;
  /** A card was chosen: its `TicketView.path`, used only as a key. */
  onSelect: (path: string) => void;
  /** The ticket id to carry the next mark (S-0023). */
  nextId?: string | null;
}

const selectClass =
  "rounded-md border border-line bg-surface-raised px-2 py-1 text-list text-ink outline-none focus-visible:ring-2 focus-visible:ring-ring";

function parsePlan(v: string): PlanFilter {
  if (v === "all") return { kind: "all" };
  if (v === "inbox") return { kind: "inbox" };
  return { kind: "plan", name: v.slice("plan:".length) };
}

function parseRepo(v: string): RepoFilter {
  if (v === "all") return { kind: "all" };
  if (v === "none") return { kind: "none" };
  return { kind: "repo", name: v.slice("repo:".length) };
}

/** The Build board (design D8): five columns of cards, with plan and repo filters
 *  held in local state, back to All when the workspace changes. */
export function BuildBoard({ root, onSelect, nextId }: Props) {
  const board = useBuildBoard(root);
  const [filters, setFilters] = useState<{ root: string | null; plan: string; repo: string }>({
    root,
    plan: "all",
    repo: "all",
  });
  if (filters.root !== root) setFilters({ root, plan: "all", repo: "all" });
  const f = filters.root === root ? filters : { root, plan: "all", repo: "all" };

  const shown = filterTickets(board.tickets, parsePlan(f.plan), parseRepo(f.repo));
  const columns = placeAll(shown, board.builds, board.closed);
  const noTickets = board.tickets.length === 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-3 py-2">
        <label className="flex items-center gap-2 text-label text-ink-muted">
          Plan
          <select
            className={selectClass}
            value={f.plan}
            onChange={(e) => setFilters({ ...f, plan: e.target.value })}
          >
            <option value="all">All</option>
            {planOptions(board.tickets).map((p) => (
              <option key={p.name} value={`plan:${p.name}`}>
                {p.closed ? `${p.name} (closed)` : p.name}
              </option>
            ))}
            <option value="inbox">Inbox</option>
          </select>
        </label>
        <label className="flex items-center gap-2 text-label text-ink-muted">
          Repo
          <select
            className={selectClass}
            value={f.repo}
            onChange={(e) => setFilters({ ...f, repo: e.target.value })}
          >
            <option value="all">All</option>
            {repoOptions(board.tickets).map((r) => (
              <option key={r} value={`repo:${r}`}>
                {r}
              </option>
            ))}
            <option value="none">{NO_REPO}</option>
          </select>
        </label>
      </div>
      {/* Capped, so many warnings scroll here instead of pushing the cards out. */}
      <div className="max-h-1/3 shrink-0 overflow-auto">
        <QueryFeedback warnings={board.warnings} error={board.error} />
        {board.unreadable.length > 0 && (
          <details className="px-3 py-2 text-list text-ink-muted">
            <summary className="cursor-pointer">{board.unreadable.length} unreadable</summary>
            <ul className="mt-1 flex flex-col gap-1">
              {board.unreadable.map((p, i) => (
                <li key={`${p.id}:${i}`}>
                  <span className="font-mono text-id">{p.id}</span> {p.message}
                </li>
              ))}
            </ul>
          </details>
        )}
      </div>
      {!board.loading && noTickets && board.error === null && (
        <p className="p-4 text-list text-ink-faint">No tickets yet</p>
      )}
      <div className="grid min-h-0 flex-1 grid-cols-5 gap-3 overflow-auto p-3">
        {BOARD_ORDER.map((c) => (
          <section
            key={c}
            role="group"
            aria-label={COLUMN_LABEL[c]}
            className="flex min-w-0 flex-col gap-2"
          >
            <h3
              className={`text-label font-medium uppercase tracking-[var(--tracking-label)] ${
                c === "draft" ? DRAFT_INK : "text-ink-muted"
              }`}
            >
              {COLUMN_LABEL[c]} <span className="font-mono text-ink-faint">{columns[c].length}</span>
            </h3>
            {columns[c].map((t) => (
              <Card
                key={t.path}
                ticket={t}
                info={board.builds.get(t.id)}
                next={t.id === nextId}
                onSelect={() => onSelect(t.path)}
              />
            ))}
          </section>
        ))}
      </div>
    </div>
  );
}

function Card({
  ticket: t,
  info,
  next,
  onSelect,
}: {
  ticket: TicketView;
  info: BuildInfo | undefined;
  next: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={`flex flex-col gap-1 rounded-md border border-line p-2 text-left outline-none hover:bg-surface-selected focus-visible:ring-2 focus-visible:ring-ring ${
        next ? NEXT_MARK : "bg-surface-raised"
      }`}
    >
      <span className="flex items-baseline gap-2 text-list text-ink">
        <span className="font-mono text-id text-ink-secondary">{t.id}</span>
        <span className="min-w-0 flex-1">{t.title}</span>
      </span>
      <span className="flex flex-wrap items-center gap-2 text-label text-ink-faint">
        <span>{t.type}</span>
        {t.repo !== "" && <span>{t.repo}</span>}
        {t.dependsOn.length > 0 && <span>after {t.dependsOn.join(", ")}</span>}
      </span>
      <Badges ticket={t} />
      {info && (
        <span className="text-label text-ink-muted">
          {info.build.phase} · {info.build.status}
        </span>
      )}
    </button>
  );
}
