import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { build, fail, ok, status, ticket } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { TicketView } from "@/generated/TicketView";
import { Viewer } from "./Viewer";

const BODY = [
  "# Heading",
  "",
  "| a | b |",
  "|---|---|",
  "| 1 | 2 |",
  "",
  "```",
  "let x = 1;",
  "```",
  "",
  "<script>window.__ran = true</script>",
  "",
  "bell \\x07 here",
].join("\n");

/** A success envelope carrying core's warnings. */
const warns = (data: unknown, warnings: string[]) => ({ schema: 1, data, warnings });

/** Serve status, tickets and read_ticket; by default the read echoes the list's body,
 *  so the fallback and the read agree. */
function serve(
  list: TicketView[],
  over: {
    status?: unknown;
    tickets?: unknown;
    readText?: (rel: string) => unknown;
  } = {},
) {
  const s = status([{ repo: "daiku", holder: "S-1", builds: [build("S-1")] }]);
  const readText =
    over.readText ?? ((rel: string) => ok(list.find((t) => t.path === rel)?.body ?? ""));
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === "status") return over.status ?? ok(s);
    if (cmd === "read_ticket") return readText((args as { path: string }).path);
    return over.tickets ?? ok({ tickets: list, unreadable: [] });
  });
}

function show(key: string, onClose = vi.fn()) {
  const client = makeQueryClient();
  const view = render(
    <QueryClientProvider client={client}>
      <Viewer
        root="/w"
        selected={{ kind: "ticket", key }}
        focused={false}
        onFill={() => {}}
        onClose={onClose}
      />
    </QueryClientProvider>,
  );
  return { ...view, client };
}

describe("TicketPanel", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows fields, badges, the build and its lock, and reads the ticket's file", async () => {
    const t = ticket("S-1", {
      body: "Hello",
      dependsOn: ["S-0"],
      requirements: ["R1", "R2"],
      labels: ["app"],
      external: { jira: "ABC-1" },
      drifted: true,
      frozen: true,
      status: "published",
    });
    serve([t]);
    const { container } = show(t.path);
    await screen.findByText("Hello");
    const text = container.textContent ?? "";
    expect(text).toContain("S-0");
    expect(text).toContain("R1, R2");
    expect(text).toContain("jira=ABC-1");
    expect(text).toContain("4-plan");
    expect(screen.getByText("drifted")).toBeTruthy();
    expect(screen.getByText("frozen")).toBeTruthy();
    expect(screen.getByText(/Build: implement · in-progress/).textContent).toContain(
      "holds the lock on daiku",
    );
    expect(screen.getByText("S-1 · Title S-1")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("read_ticket", { root: "/w", path: t.path });
  });

  it("renders the read body through Markdown: table and code, no script, escapes visible", async () => {
    const t = ticket("S-2", { body: "stale list body" });
    serve([t], { readText: () => ok(BODY) });
    const { container } = show(t.path);
    await screen.findByText("Heading");
    expect(container.querySelector("table")).not.toBeNull();
    expect(container.querySelector("pre code")?.textContent).toContain("let x = 1;");
    expect(container.querySelector("script")).toBeNull();
    expect((window as unknown as { __ran?: boolean }).__ran).toBeUndefined();
    expect(container.textContent).toContain("\\x07");
    expect(container.textContent).not.toContain("\u0007");
    expect(container.textContent).not.toContain("stale list body");
  });

  it("strips the read text's front matter, which the fields already show", async () => {
    const t = ticket("S-4", { body: "stale list body" });
    serve([t], { readText: () => ok("---\nfront-matter-marker: yes\n---\n\n# Fresh body") });
    const { container } = show(t.path);
    await screen.findByText("Fresh body");
    expect(container.textContent).not.toContain("stale list body");
    expect(container.textContent).not.toContain("front-matter-marker");
  });

  it("shows the current body after a ticket edit refetches the read", async () => {
    const t = ticket("S-5", { body: "ignored list body" });
    let current = "First body";
    serve([t], { readText: () => ok(current) });
    const { client } = show(t.path);
    await screen.findByText("First body");

    current = "Second body";
    await client.refetchQueries();
    expect(await screen.findByText("Second body")).toBeTruthy();
    expect(screen.queryByText("First body")).toBeNull();
  });

  it("shows board and read warnings in query order, beside the body", async () => {
    const t = ticket("S-6");
    const s = status([{ repo: "daiku", holder: "S-1", builds: [build("S-1")] }]);
    serve([t], {
      status: warns(s, ["status warning"]),
      tickets: warns({ tickets: [t], unreadable: [] }, ["tickets warning"]),
      readText: () => warns("# Body", ["read warning"]),
    });
    show(t.path);
    await screen.findByText("Body");
    const list = screen.getByText("status warning").closest("ul") as HTMLElement;
    expect(within(list).getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      "status warning",
      "tickets warning",
      "read warning",
    ]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows a read failure as Kind: message, keeping the list body", async () => {
    const t = ticket("S-7", { body: "Fallback body" });
    serve([t], { readText: () => fail("Syntax", "unexpected end of file") });
    show(t.path);
    expect((await screen.findByRole("alert")).textContent).toBe("Syntax: unexpected end of file");
    expect(screen.getByText("Fallback body")).toBeTruthy();
  });

  it("keeps the last good body and metadata through a failed refetch, then recovers", async () => {
    const t = ticket("S-8", { body: "ignored list body" });
    let reply: unknown = ok("Initial body");
    serve([t], { readText: () => reply });
    const { client } = show(t.path);
    await screen.findByText("Initial body");
    expect(screen.getByText("S-8 · Title S-8")).toBeTruthy();

    reply = fail("Io", "disk went away");
    await client.refetchQueries();
    expect((await screen.findByRole("alert")).textContent).toBe("Io: disk went away");
    expect(screen.getByText("Initial body")).toBeTruthy();
    expect(screen.getByText("S-8 · Title S-8")).toBeTruthy();

    reply = ok("Recovered body");
    await client.refetchQueries();
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByText("Recovered body")).toBeTruthy();
    expect(screen.queryByText("Initial body")).toBeNull();
  });

  it("says when the ticket is no longer in the workspace", async () => {
    serve([ticket("S-3")]);
    show("plan/gone.md");
    expect(await screen.findByText("This ticket is no longer in the workspace")).toBeTruthy();
  });

  it("closes through the pane header", async () => {
    const t = ticket("S-1");
    serve([t]);
    const onClose = vi.fn();
    show(t.path, onClose);
    fireEvent.click(await screen.findByRole("button", { name: "Close file" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
