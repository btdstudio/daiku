import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { adr, fail, idea, ok, plan, planStatus } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { Record as Adr } from "@/generated/AdrRecord";
import type { Idea } from "@/generated/Idea";
import type { Status } from "@/generated/Status";
import { PlanList } from "./PlanList";
import { usePlanTab } from "./usePlanTab";

function serve(s: unknown, ideas: Idea[] | unknown, adrs: Adr[] | unknown) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "status") return s;
    if (cmd === "idea_list") return Array.isArray(ideas) ? ok(ideas) : ideas;
    return Array.isArray(adrs) ? ok(adrs) : adrs;
  });
}

function show(onSelect = vi.fn(), root = "/w") {
  const client = makeQueryClient();
  const ui = (r: string) => (
    <QueryClientProvider client={client}>
      <PlanList root={r} onFill={() => {}} onSelect={onSelect} />
    </QueryClientProvider>
  );
  const view = render(ui(root));
  return { ...view, client, rerootTo: (r: string) => view.rerender(ui(r)) };
}

const group = (name: string) => screen.getByRole("group", { name });
const st = (names: string[]): Status => planStatus(names.map((n) => plan(n)));

describe("PlanList", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows Plans, Ideas and ADRs in order with counts, and References as later", async () => {
    serve(
      ok(st(["2-b", "1-a"])),
      [idea(2, { status: "parked" }), idea(1)],
      [adr(1), adr(3), adr(2)],
    );
    show();
    await screen.findByText("1-a");
    expect(screen.getAllByRole("group").map((g) => g.getAttribute("aria-label"))).toEqual([
      "Plans",
      "Ideas",
      "ADRs",
    ]);
    // Plans sorted by name; ideas by id; ADRs newest first.
    expect(within(group("Plans")).getAllByRole("button").map((b) => b.textContent)).toEqual([
      "1-aintake",
      "2-bintake",
    ]);
    expect(within(group("Ideas")).getByText("Idea 1")).toBeTruthy();
    expect(
      within(group("ADRs")).getAllByRole("button").map((b) => b.textContent?.slice(0, 4)),
    ).toEqual(["0003", "0002", "0001"]);
    expect(screen.getByText("References · later")).toBeTruthy();
    expect(screen.queryByRole("group", { name: "References" })).toBeNull();
    expect(within(group("Ideas")).getByText("parked").className).toContain("text-ink-faint");
    expect(within(group("Ideas")).getByText("Idea 2").parentElement?.className).toContain(
      "text-ink-faint",
    );
  });

  it("folds a group at 8 rows, expands in place, and folds again on a new workspace", async () => {
    const many = Array.from({ length: 11 }, (_, i) => idea(i + 1));
    serve(ok(st([])), many, []);
    const { rerootTo } = show();
    await screen.findByText("Idea 1");
    expect(within(group("Ideas")).getAllByRole("button")).toHaveLength(9);
    expect(screen.queryByText("Idea 9")).toBeNull();
    fireEvent.click(screen.getByText("3 more"));
    expect(screen.getByText("Idea 11")).toBeTruthy();
    expect(screen.queryByText("3 more")).toBeNull();
    rerootTo("/other");
    await waitFor(() => expect(screen.getByText("3 more")).toBeTruthy());
  });

  it("says an empty group is empty", async () => {
    serve(ok(st([])), [], []);
    show();
    expect(await screen.findAllByText("Nothing here yet")).toHaveLength(3);
  });

  it("selects a plan, an idea and an ADR", async () => {
    serve(ok(st(["1-a"])), [idea(7)], [adr(9)]);
    const onSelect = vi.fn();
    show(onSelect);
    fireEvent.click(await screen.findByText("1-a"));
    fireEvent.click(screen.getByText("Idea 7"));
    fireEvent.click(screen.getByText("Decision 9"));
    expect(onSelect.mock.calls.map((c) => c[0])).toEqual([
      { kind: "plan", name: "1-a" },
      { kind: "idea", id: 7 },
      { kind: "adr", number: 9 },
    ]);
  });

  it("shows an error over the last data after a failed refetch", async () => {
    serve(ok(st(["1-a"])), [idea(1)], [adr(1)]);
    const { client } = show();
    await screen.findByText("1-a");
    serve(ok(st(["1-a"])), fail("Io", "disk went away"), [adr(1)]);
    await client.refetchQueries();
    expect((await screen.findByRole("alert")).textContent).toContain("disk went away");
    expect(screen.getByText("Idea 1")).toBeTruthy();
    expect(screen.getByText("1-a")).toBeTruthy();
  });
});

describe("usePlanTab counts", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("counts whole groups, keyed by the rail's group names, with References 0", async () => {
    serve(
      ok(st(["1-a", "2-b"])),
      Array.from({ length: 10 }, (_, i) => idea(i + 1)),
      [adr(1)],
    );
    let seen: Record<string, number> = {};
    function Probe() {
      seen = usePlanTab("/w").counts;
      return null;
    }
    render(
      <QueryClientProvider client={makeQueryClient()}>
        <Probe />
      </QueryClientProvider>,
    );
    await waitFor(() => expect(seen.Ideas).toBe(10));
    expect(seen).toEqual({ Plans: 2, Ideas: 10, ADRs: 1, References: 0 });
  });
});
