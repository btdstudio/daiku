import { type BuildInfo, type Column, columnFor } from "@/board/columns";
import type { TicketView } from "@/generated/TicketView";

/** Rows a list group shows before its "N more" row. */
export const FOLD_AT = 8;

export type ListColumn = "inBuild" | "ready" | "draft" | "done";
export const LIST_ORDER: ListColumn[] = ["inBuild", "ready", "draft", "done"];
/** The board's columns, left to right. */
export const BOARD_ORDER: Column[] = ["draft", "ready", "inBuild", "done", "closed"];

export const COLUMN_LABEL: Record<Column, string> = {
  inBuild: "In build",
  ready: "Ready",
  draft: "Draft",
  done: "Done",
  closed: "Closed",
};

export interface Group {
  column: ListColumn;
  tickets: TicketView[];
}

const byId = (a: TicketView, b: TicketView) =>
  a.id < b.id ? -1 : a.id > b.id ? 1 : a.path < b.path ? -1 : a.path > b.path ? 1 : 0;

/** Every ticket placed by `columnFor`, each column sorted by id; Done newest build
 *  first (`updatedAt` is an ISO timestamp), then id. */
export function placeAll(
  tickets: TicketView[],
  builds: Map<string, BuildInfo>,
  closed: Set<string>,
): Record<Column, TicketView[]> {
  const out: Record<Column, TicketView[]> = {
    inBuild: [],
    ready: [],
    draft: [],
    done: [],
    closed: [],
  };
  for (const t of tickets) {
    const c = columnFor(t, builds, closed);
    if (c !== null) out[c].push(t);
  }
  for (const c of Object.keys(out) as Column[]) out[c].sort(byId);
  out.done.sort((a, b) => {
    const ua = builds.get(a.id)?.build.updatedAt ?? "";
    const ub = builds.get(b.id)?.build.updatedAt ?? "";
    return ua < ub ? 1 : ua > ub ? -1 : byId(a, b);
  });
  return out;
}

/** The four list groups, in order; Closed isn't listed. */
export function groupsFor(
  tickets: TicketView[],
  builds: Map<string, BuildInfo>,
  closed: Set<string>,
): Group[] {
  const placed = placeAll(tickets, builds, closed);
  return LIST_ORDER.map((column) => ({ column, tickets: placed[column] }));
}

export type Row<T> = { kind: "row"; item: T } | { kind: "more"; count: number };

/** The first `FOLD_AT` rows and an "N more" row, or every row once expanded. */
export function visibleRows<T>(rows: T[], expanded: boolean): Row<T>[] {
  const shown = expanded ? rows : rows.slice(0, FOLD_AT);
  const out: Row<T>[] = shown.map((item) => ({ kind: "row", item }));
  if (!expanded && rows.length > FOLD_AT) {
    out.push({ kind: "more", count: rows.length - FOLD_AT });
  }
  return out;
}

export const NO_REPO = "(no repo)";

/** Plan filter: `all`, `inbox` (change is ""), or a plan name. */
export type PlanFilter = { kind: "all" } | { kind: "inbox" } | { kind: "plan"; name: string };
/** Repo filter: `all`, a repo name, or the tickets with no repo. */
export type RepoFilter = { kind: "all" } | { kind: "none" } | { kind: "repo"; name: string };

export function filterTickets(
  tickets: TicketView[],
  plan: PlanFilter,
  repo: RepoFilter,
): TicketView[] {
  return tickets.filter((t) => {
    if (plan.kind === "inbox" && t.change !== "") return false;
    if (plan.kind === "plan" && t.change !== plan.name) return false;
    if (repo.kind === "none" && t.repo !== "") return false;
    if (repo.kind === "repo" && t.repo !== repo.name) return false;
    return true;
  });
}

export interface PlanOption {
  name: string;
  closed: boolean;
}

/** The plans seen among the tickets, by name, with whether each is closed. */
export function planOptions(tickets: TicketView[]): PlanOption[] {
  const seen = new Map<string, boolean>();
  for (const t of tickets) {
    if (t.change !== "" && !seen.has(t.change)) seen.set(t.change, t.changeClosed);
  }
  return [...seen].map(([name, closed]) => ({ name, closed })).sort((a, b) => (a.name < b.name ? -1 : 1));
}

export function repoOptions(tickets: TicketView[]): string[] {
  return [...new Set(tickets.map((t) => t.repo).filter((r) => r !== ""))].sort();
}
