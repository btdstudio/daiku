import { describe, expect, it } from "vitest";
import { buildIndex } from "@/board/columns";
import { build, status, ticket } from "@/board/fixtures";
import {
  FOLD_AT,
  filterTickets,
  groupsFor,
  planOptions,
  repoOptions,
  visibleRows,
} from "./groups";

describe("groupsFor", () => {
  const tickets = [
    ticket("S-3", { status: "draft" }),
    ticket("S-1", { status: "ready" }),
    ticket("S-2", { status: "draft" }),
    ticket("S-4"),
    ticket("S-5"),
    ticket("S-6"),
    ticket("S-7", { status: "closed" }),
    ticket("S-8"),
  ];
  const s = status(
    [
      {
        repo: "daiku",
        builds: [
          build("S-4"),
          build("S-5", { gates: { ship: "complete" }, updatedAt: "2026-01-02T00:00:00.000Z" }),
          build("S-6", { gates: { ship: "complete" }, updatedAt: "2026-01-05T00:00:00.000Z" }),
        ],
      },
    ],
    ["S-8"],
  );
  const groups = groupsFor(tickets, buildIndex(s), new Set(s.closedBuilds));
  const ids = (c: string) => groups.find((g) => g.column === c)?.tickets.map((t) => t.id);

  it("has the four groups in order and leaves out closed and unplaced tickets", () => {
    expect(groups.map((g) => g.column)).toEqual(["inBuild", "ready", "draft", "done"]);
    const all = groups.flatMap((g) => g.tickets.map((t) => t.id));
    expect(all).not.toContain("S-8");
    expect(all).not.toContain("S-7");
  });

  it("sorts by id, and Done newest build first", () => {
    expect(ids("draft")).toEqual(["S-2", "S-3"]);
    expect(ids("inBuild")).toEqual(["S-4"]);
    expect(ids("done")).toEqual(["S-6", "S-5"]);
  });
});

describe("visibleRows", () => {
  const rows = Array.from({ length: 11 }, (_, i) => i);

  it("shows FOLD_AT rows and a more row", () => {
    expect(FOLD_AT).toBe(8);
    const v = visibleRows(rows, false);
    expect(v).toHaveLength(9);
    expect(v[8]).toEqual({ kind: "more", count: 3 });
  });

  it("shows every row, and no more row, once expanded", () => {
    const v = visibleRows(rows, true);
    expect(v).toHaveLength(11);
    expect(v.every((r) => r.kind === "row")).toBe(true);
  });

  it("shows exactly FOLD_AT rows with no more row", () => {
    expect(visibleRows(rows.slice(0, 8), false)).toHaveLength(8);
  });
});

describe("filters", () => {
  const tickets = [
    ticket("S-1", { change: "4-plan", repo: "daiku" }),
    ticket("S-2", { change: "5-old", changeClosed: true, repo: "" }),
    ticket("N-1", { change: "", inbox: true, repo: "api" }),
  ];
  const all = { kind: "all" } as const;
  const idsOf = (p: Parameters<typeof filterTickets>[1], r: Parameters<typeof filterTickets>[2]) =>
    filterTickets(tickets, p, r).map((t) => t.id);

  it("filters by plan, inbox and repo", () => {
    expect(idsOf(all, all)).toEqual(["S-1", "S-2", "N-1"]);
    expect(idsOf({ kind: "plan", name: "4-plan" }, all)).toEqual(["S-1"]);
    expect(idsOf({ kind: "inbox" }, all)).toEqual(["N-1"]);
    expect(idsOf(all, { kind: "repo", name: "api" })).toEqual(["N-1"]);
    expect(idsOf(all, { kind: "none" })).toEqual(["S-2"]);
    expect(idsOf({ kind: "plan", name: "4-plan" }, { kind: "repo", name: "api" })).toEqual([]);
  });

  it("lists the plans and repos seen, marking closed plans", () => {
    expect(planOptions(tickets)).toEqual([
      { name: "4-plan", closed: false },
      { name: "5-old", closed: true },
    ]);
    expect(repoOptions(tickets)).toEqual(["api", "daiku"]);
  });
});
