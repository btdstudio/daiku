import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { build, ok, status, ticket } from "@/board/fixtures";
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

function show(list: TicketView[], key: string, onClose = vi.fn()) {
  const s = status([{ repo: "daiku", holder: "S-1", builds: [build("S-1")] }]);
  vi.mocked(invoke).mockImplementation(async (cmd: string) =>
    cmd === "status" ? ok(s) : ok({ tickets: list, unreadable: [] }),
  );
  return render(
    <QueryClientProvider client={makeQueryClient()}>
      <Viewer
        root="/w"
        selected={{ kind: "ticket", key }}
        focused={false}
        onFill={() => {}}
        onClose={onClose}
      />
    </QueryClientProvider>,
  );
}

describe("TicketPanel", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows fields, badges, the build and its lock, and reads no file", async () => {
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
    const { container } = show([t], t.path);
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
    expect(vi.mocked(invoke).mock.calls.map((c) => c[0])).not.toContain("read_text");
  });

  it("renders the body through Markdown: table and code, no script, escapes visible", async () => {
    const t = ticket("S-2", { body: BODY });
    const { container } = show([t], t.path);
    await screen.findByText("Heading");
    expect(container.querySelector("table")).not.toBeNull();
    expect(container.querySelector("pre code")?.textContent).toContain("let x = 1;");
    expect(container.querySelector("script")).toBeNull();
    expect((window as unknown as { __ran?: boolean }).__ran).toBeUndefined();
    expect(container.textContent).toContain("\\x07");
    expect(container.textContent).not.toContain("\u0007");
  });

  it("says when the ticket is no longer in the workspace", async () => {
    show([ticket("S-3")], "plan/gone.md");
    expect(await screen.findByText("This ticket is no longer in the workspace")).toBeTruthy();
  });

  it("closes through the pane header", async () => {
    const t = ticket("S-1");
    const onClose = vi.fn();
    show([t], t.path, onClose);
    fireEvent.click(await screen.findByRole("button", { name: "Close file" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
