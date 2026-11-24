import type { NextItem } from "@/generated/NextItem";
import type { Fold } from "./fold";

const HOUR = 3_600_000;

/** " · 25h" or " · 3d" when `since` is more than 24 hours before `now`, else "". A
 *  missing or unparseable `since` gives "". Whole hours under 48h, then whole days,
 *  rounded down. */
export function age(since: string | null | undefined, now: number): string {
  if (!since) return "";
  const t = Date.parse(since);
  if (Number.isNaN(t)) return "";
  const hours = (now - t) / HOUR;
  if (hours <= 24) return "";
  if (hours < 48) return ` · ${Math.floor(hours)}h`;
  return ` · ${Math.floor(hours / 24)}d`;
}

/** The nudge's text for the first item (R5), with its age. `undefined` means the list
 *  loaded and is empty; `null` means no data yet (or ever), so no text. Plain text,
 *  never HTML. */
export function nudgeText(
  item: NextItem | undefined | null,
  ticketStatus: string | null,
  fold: Fold,
  now: number,
): string | null {
  if (item === null) return null;
  if (item === undefined) return "Nothing is waiting on you";
  const short = fold === "narrow";
  const a = age(item.since, now);
  switch (item.kind) {
    case "build": {
      const id = item.ticket;
      const status = ticketStatus ?? "";
      if (short) return `Next: ${id}${status ? ` · ${status}` : ""}${a}`;
      const is = status === "" ? "" : status === "draft" ? " is a draft" : ` is ${status}`;
      return `Next: ${id}${is}. Start its build when you're ready.${a}`;
    }
    case "approve":
      return short
        ? `Next: ${item.change} · ${item.phase}${a}`
        : `Next: ${item.change} ${item.phase} is waiting for your approval${a}`;
    case "start":
      return short
        ? `Next: ${item.change} · start ${item.phase}${a}`
        : `Next: start ${item.phase} for ${item.change}${a}`;
    default:
      return "Next: something needs you";
  }
}
