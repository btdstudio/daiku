import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { makeQueryClient } from "@/data/queryClient";
import { BuildBoard } from "./BuildBoard";
import { build, ok, status, ticket } from "./fixtures";

const all = [
  ticket("S-1", { status: "draft", repo: "daiku" }),
  ticket("S-2", { status: "ready", repo: "daiku", drifted: true }),
  ticket("S-3", { status: "published", repo: "api", frozen: true, dependsOn: ["S-1"] }),
  ticket("S-4", { repo: "daiku" }),
  ticket("S-5", { repo: "daiku" }),
  ticket("S-6", { repo: "daiku" }),
  ticket("N-1", { change: "", inbox: true, repo: "", status: "draft", type: "task" }),
  ticket("O-1", { change: "1-old", changeClosed: true, status: "draft" }),
];
const s = status(
  [
    {
      repo: "daiku",
      builds: [
        build("S-4", { phase: "implement", status: "in-progress" }),
        build("S-5", { phase: "ship", status: "complete", gates: { ship: "complete" } }),
      ],
    },
  ],
  ["S-6"],
);

function show(root = "/w", onSelect = vi.fn()) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) =>
    cmd === "status" ? ok(s) : ok({ tickets: all, unreadable: [] }),
  );
  const client = makeQueryClient();
  const ui = (r: string) => (
    <QueryClientProvider client={client}>
      <BuildBoard root={r} onSelect={onSelect} />
    </QueryClientProvider>
  );
  const view = render(ui(root));
  return { rerootTo: (r: string) => view.rerender(ui(r)) };
}

const col = (name: string) => screen.getByRole("group", { name });
const ids = (name: string) =>
  within(col(name))
    .queryAllByRole("button")
    .map((b) => b.querySelector("span.font-mono")?.textContent);
const planSel = () => screen.getByLabelText("Plan") as HTMLSelectElement;
const repoSel = () => screen.getByLabelText("Repo") as HTMLSelectElement;

describe("BuildBoard", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows five columns with every ticket placed by the rule", async () => {
    show();
    await screen.findByText("Title S-1");
    expect(screen.getAllByRole("group").map((g) => g.getAttribute("aria-label"))).toEqual([
      "Draft",
      "Ready",
      "In build",
      "Done",
      "Closed",
    ]);
    expect(ids("Draft")).toEqual(["N-1", "O-1", "S-1"]);
    expect(ids("Ready")).toEqual(["S-2", "S-3"]);
    expect(ids("In build")).toEqual(["S-4"]);
    expect(ids("Done")).toEqual(["S-5"]);
    expect(ids("Closed")).toEqual(["S-6"]);
  });

  it("shows badges, dependencies and the build's phase and status on a card", async () => {
    show();
    await screen.findByText("Title S-1");
    expect(within(col("Ready")).getByText("drifted")).toBeTruthy();
    expect(within(col("Ready")).getByText("published")).toBeTruthy();
    expect(within(col("Ready")).getByText("frozen")).toBeTruthy();
    expect(within(col("Ready")).getByText("after S-1")).toBeTruthy();
    expect(within(col("In build")).getByText("implement · in-progress")).toBeTruthy();
  });

  it("filters by plan, inbox and repo, and resets on a new workspace", async () => {
    const { rerootTo } = show();
    await screen.findByText("Title S-1");
    expect(within(planSel()).getByText("1-old (closed)")).toBeTruthy();

    fireEvent.change(planSel(), { target: { value: "inbox" } });
    expect(ids("Draft")).toEqual(["N-1"]);
    expect(ids("Ready")).toEqual([]);

    fireEvent.change(planSel(), { target: { value: "plan:4-plan" } });
    fireEvent.change(repoSel(), { target: { value: "repo:api" } });
    expect(ids("Ready")).toEqual(["S-3"]);
    expect(ids("Draft")).toEqual([]);

    fireEvent.change(planSel(), { target: { value: "all" } });
    fireEvent.change(repoSel(), { target: { value: "none" } });
    expect(ids("Draft")).toEqual(["N-1"]);

    rerootTo("/other");
    await waitFor(() => expect(planSel().value).toBe("all"));
    expect(repoSel().value).toBe("all");
  });

  it("selects a ticket by its path when its card is clicked", async () => {
    const onSelect = vi.fn();
    show("/w", onSelect);
    fireEvent.click(await screen.findByText("Title S-4"));
    expect(onSelect).toHaveBeenCalledWith("plan/specs/4-plan/tickets/S-4.md");
  });
});
