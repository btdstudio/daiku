import { columnFor } from "@/board/columns";
import { useBuildBoard } from "@/build/useBuildBoard";
import { useNext, useTickets } from "@/data/hooks";
import type { NextItem } from "@/generated/NextItem";
import type { Selected } from "@/viewer/selected";
import type { Fold } from "./fold";
import { nudgeText } from "./nudge";
import type { Tab } from "./viewState";

export interface Nudge {
  /** The first item, or undefined when nothing is waiting (or nothing loaded yet). */
  item: NextItem | undefined;
  /** The status-bar text, or null before `next` has loaded. */
  text: string | null;
  /** The ticket id of the first `build` item, for the next mark. */
  nextTicket: string | null;
  /** The name of the first plan-loop approve or start item, for the next mark. */
  nextPlan: string | null;
  /** The Build list group holding the marked ticket (the rail's highlighted count). */
  buildGroup: string | undefined;
  /** "Plans" when a plan is marked. */
  planGroup: string | undefined;
  /** Every warning from `next` and `tickets`, `next` first (`S-0019`); the item, marks
   *  and targets are unchanged by it. */
  warnings: string[];
  /** Where clicking the item goes, or null when it goes nowhere. */
  target: (item: NextItem | undefined) => { tab: Tab; sel: Selected } | null;
}

/** Shared, so a nudge without warnings doesn't allocate one each render. */
const NO_WARNINGS: string[] = [];

/** The nudge (S-0023 D4): `next` and `tickets` on every tab, the first item's text and
 *  its marked targets. */
export function useNudge(root: string | null, fold: Fold): Nudge {
  const next = useNext(root);
  const tickets = useTickets(root);
  // "Now" is when `next` last loaded: reading the clock in render is impure, and the age
  // is in whole hours, so a focus refetch (which reloads it) keeps it current enough.
  const now = next.dataUpdatedAt;
  const items = next.data?.items;
  const item = items?.[0];
  const list = tickets.data?.tickets;

  const pathOf = (id: string) => list?.find((t) => t.id === id)?.path ?? null;
  const statusOf = (it: NextItem | undefined) =>
    it?.kind === "build" ? (list?.find((t) => t.id === it.ticket)?.status ?? null) : null;

  const firstBuild = items?.find((i) => i.kind === "build");
  const nextTicket = firstBuild?.kind === "build" ? firstBuild.ticket : null;
  const firstPlan = items?.find(
    (i) => (i.kind === "approve" || i.kind === "start") && i.loop === "plan",
  );
  const nextPlan = firstPlan && firstPlan.kind !== "build" ? firstPlan.change : null;

  const board = useBuildBoard(nextTicket !== null ? root : null);
  const marked = nextTicket !== null ? board.tickets.find((t) => t.id === nextTicket) : undefined;
  const column = marked ? columnFor(marked, board.builds, board.closed) : null;
  const buildGroup = column !== null && column !== "closed" ? column : undefined;

  const target: Nudge["target"] = (it) => {
    if (!it) return null;
    const ticket = (id: string) => {
      const key = pathOf(id);
      return key === null ? null : { tab: "build" as const, sel: { kind: "ticket" as const, key } };
    };
    switch (it.kind) {
      case "build":
        return ticket(it.ticket);
      case "approve":
      case "start":
        if (it.loop === "plan") {
          return { tab: "plan", sel: { kind: "plan", name: it.change, phase: it.phase } };
        }
        return it.loop === "build" ? ticket(it.change) : null;
      default:
        return null;
    }
  };

  return {
    item,
    text: nudgeText(items ? item : null, statusOf(item), fold, now),
    nextTicket,
    nextPlan,
    buildGroup,
    planGroup: nextPlan !== null ? "Plans" : undefined,
    warnings:
      next.warnings.length > 0 || tickets.warnings.length > 0
        ? [...next.warnings, ...tickets.warnings]
        : NO_WARNINGS,
    target,
  };
}
