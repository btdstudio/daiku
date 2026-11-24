import { useMemo } from "react";
import { type BuildInfo, buildIndex } from "@/board/columns";
import type { Column } from "@/board/columns";
import { FailureError } from "@/data/envelope";
import { useStatus, useTickets } from "@/data/hooks";
import type { Problem } from "@/generated/Problem";
import type { TicketView } from "@/generated/TicketView";
import { type Group, type ListColumn, groupsFor, placeAll } from "./groups";

export interface BuildBoard {
  tickets: TicketView[];
  unreadable: Problem[];
  builds: Map<string, BuildInfo>;
  closed: Set<string>;
  groups: Group[];
  /** Every ticket placed in its column (all five), unfiltered. */
  columns: Record<Column, TicketView[]>;
  counts: Record<ListColumn, number>;
  /** The first failure of either query; the last data stays in the other fields. */
  error: string | null;
  loading: boolean;
}

const EMPTY_TICKETS: TicketView[] = [];
const EMPTY_PROBLEMS: Problem[] = [];

function message(e: unknown): string {
  return e instanceof FailureError ? e.failure.error.message : String(e);
}

/** The two queries joined once: tickets placed in columns by build state (D6). A
 *  failed refetch keeps the last data, with `error` set. */
export function useBuildBoard(root: string | null): BuildBoard {
  const status = useStatus(root);
  const tickets = useTickets(root);
  const statusData = status.data;
  const ticketData = tickets.data;

  const joined = useMemo(() => {
    const list = ticketData?.tickets ?? EMPTY_TICKETS;
    const builds = statusData ? buildIndex(statusData) : new Map<string, BuildInfo>();
    const closed = new Set(statusData?.closedBuilds ?? []);
    const columns = placeAll(list, builds, closed);
    const groups = groupsFor(list, builds, closed);
    const counts = Object.fromEntries(groups.map((g) => [g.column, g.tickets.length])) as Record<
      ListColumn,
      number
    >;
    return { list, builds, closed, columns, groups, counts };
  }, [statusData, ticketData]);

  const err = status.error ?? tickets.error;
  return {
    tickets: joined.list,
    unreadable: ticketData?.unreadable ?? EMPTY_PROBLEMS,
    builds: joined.builds,
    closed: joined.closed,
    groups: joined.groups,
    columns: joined.columns,
    counts: joined.counts,
    error: err ? message(err) : null,
    loading: root !== null && (status.isPending || tickets.isPending) && !err,
  };
}
