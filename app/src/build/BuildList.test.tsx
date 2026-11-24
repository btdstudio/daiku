import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { build, fail, ok, status, ticket } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { Status } from "@/generated/Status";
import type { TicketView } from "@/generated/TicketView";
import { BuildList } from "./BuildList";

type Reply = unknown;
function serve(s: Reply, t: Reply) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "status" ? s : t));
}
const tickets = (list: TicketView[], unreadable: unknown[] = []) =>
  ok({ tickets: list, unreadable });

function show(onSelect = vi.fn(), root = "/w") {
  const client = makeQueryClient();
  const ui = (r: string) => (
    <QueryClientProvider client={client}>
      <BuildList root={r} onFill={() => {}} onSelect={onSelect} />
    </QueryClientProvider>
  );
  const view = render(ui(root));
  return { ...view, client, rerootTo: (r: string) => view.rerender(ui(r)) };
}

const group = (name: string) => screen.getByRole("group", { name });

describe("BuildList", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows the four groups with counts, and not the closed one", async () => {
    const s: Status = status(
      [{ repo: "daiku", builds: [build("S-1"), build("S-2", { gates: { ship: "complete" } })] }],
      ["S-3"],
    );
    serve(
      ok(s),
      tickets([
        ticket("S-1"),
        ticket("S-2"),
        ticket("S-3"),
        ticket("S-4", { status: "ready" }),
        ticket("S-5", { status: "draft" }),
      ]),
    );
    show();
    await screen.findByText("Title S-1");
    expect(screen.getAllByRole("group").map((g) => g.getAttribute("aria-label"))).toEqual([
      "In build",
      "Ready",
      "Draft",
      "Done",
    ]);
    expect(within(group("In build")).getByText("S-1")).toBeTruthy();
    expect(within(group("Done")).getByText("S-2")).toBeTruthy();
    expect(within(group("Ready")).getByText("S-4")).toBeTruthy();
    expect(within(group("Draft")).getByText("S-5")).toBeTruthy();
    expect(screen.queryByText("S-3")).toBeNull();
  });

  it("folds a group at 8 rows, expands in place, and folds again on a new workspace", async () => {
    const many = Array.from({ length: 11 }, (_, i) =>
      ticket(`S-${String(i + 1).padStart(2, "0")}`, { status: "draft" }),
    );
    serve(ok(status([])), tickets(many));
    const { rerootTo } = show();
    await screen.findByText("Title S-01");
    expect(within(group("Draft")).getAllByRole("button")).toHaveLength(9);
    expect(screen.queryByText("Title S-09")).toBeNull();
    fireEvent.click(screen.getByText("3 more"));
    expect(screen.getByText("Title S-11")).toBeTruthy();
    expect(screen.queryByText("3 more")).toBeNull();
    rerootTo("/other");
    await waitFor(() => expect(screen.getByText("3 more")).toBeTruthy());
  });

  it("shows the lock mark for the holder only, with its phase", async () => {
    const s = status([
      {
        repo: "daiku",
        holder: "S-1",
        builds: [build("S-1", { phase: "implement" }), build("S-2", { phase: "design" })],
      },
    ]);
    serve(ok(s), tickets([ticket("S-1"), ticket("S-2")]));
    show();
    await screen.findByText("Title S-1");
    const row = (id: string) => screen.getByText(id).closest("button") as HTMLElement;
    expect(within(row("S-1")).getByText("implement")).toBeTruthy();
    expect(within(row("S-1")).getByLabelText("repo lock")).toBeTruthy();
    expect(within(row("S-1")).getAllByText("daiku").length).toBeGreaterThan(0);
    expect(within(row("S-2")).getByText("design")).toBeTruthy();
    expect(within(row("S-2")).queryByLabelText("repo lock")).toBeNull();
  });

  it("selects a ticket by its path", async () => {
    serve(ok(status([])), tickets([ticket("S-1")]));
    const onSelect = vi.fn();
    show(onSelect);
    fireEvent.click(await screen.findByText("Title S-1"));
    expect(onSelect).toHaveBeenCalledWith("plan/specs/4-plan/tickets/S-1.md");
  });

  it("shows an error over the last data after a failed refetch", async () => {
    serve(ok(status([])), tickets([ticket("S-1")]));
    const { client } = show();
    await screen.findByText("Title S-1");
    serve(fail("Io", "disk went away"), tickets([ticket("S-1")]));
    await client.refetchQueries();
    expect((await screen.findByRole("alert")).textContent).toContain("disk went away");
    expect(screen.getByText("Title S-1")).toBeTruthy();
  });

  it("shows an unreadable count that expands to the messages", async () => {
    serve(
      ok(status([])),
      tickets([ticket("S-1")], [{ id: "S-9", message: "bad front matter", reference: "" }]),
    );
    show();
    await screen.findByText("1 unreadable");
    const details = screen.getByText("1 unreadable").closest("details") as HTMLDetailsElement;
    expect(details.open).toBe(false);
    fireEvent.click(screen.getByText("1 unreadable"));
    expect(screen.getByText(/bad front matter/)).toBeTruthy();
  });

  it("says there are no tickets yet", async () => {
    serve(ok(status([])), tickets([]));
    show();
    expect(await screen.findByText("No tickets yet")).toBeTruthy();
  });
});
