import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { BuildBoard } from "@/board/BuildBoard";
import { ok, status, ticket } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import { Viewer } from "@/viewer/Viewer";
import { BuildList } from "./BuildList";
import { StatusText } from "./StatusText";

const INK = "text-attention-ink";
const tickets = [ticket("S-1", { status: "draft" }), ticket("S-2", { status: "ready" })];

function serve() {
  vi.mocked(invoke).mockImplementation(async (cmd: string) =>
    cmd === "status" ? ok(status([])) : ok({ tickets, unreadable: [] }),
  );
}
const wrap = (ui: React.ReactNode) => (
  <QueryClientProvider client={makeQueryClient()}>{ui}</QueryClientProvider>
);
const heading = (group: string) =>
  within(screen.getByRole("group", { name: group })).getByRole("heading");

describe("draft ink", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("the Build list's Draft header is amber; the others are not", async () => {
    serve();
    render(wrap(<BuildList root="/w" onFill={() => {}} onSelect={() => {}} />));
    await screen.findByText("S-1");
    expect(heading("Draft").className).toContain(INK);
    for (const g of ["Ready", "In build", "Done"]) expect(heading(g).className).not.toContain(INK);
  });

  it("the Build board's Draft column header is amber; the others are not", async () => {
    serve();
    render(wrap(<BuildBoard root="/w" onSelect={() => {}} />));
    await screen.findByText("S-1");
    expect(heading("Draft").className).toContain(INK);
    for (const g of ["Ready", "In build", "Done", "Closed"])
      expect(heading(g).className).not.toContain(INK);
  });

  it("the ticket panel's status is amber only for a draft", async () => {
    serve();
    const { unmount } = render(
      wrap(
        <Viewer
          root="/w"
          selected={{ kind: "ticket", key: tickets[0].path }}
          focused={false}
          onFill={() => {}}
        />,
      ),
    );
    expect((await screen.findByText("draft")).className).toContain(INK);
    unmount();
    render(
      wrap(
        <Viewer
          root="/w"
          selected={{ kind: "ticket", key: tickets[1].path }}
          focused={false}
          onFill={() => {}}
        />,
      ),
    );
    expect((await screen.findByText("ready")).className).not.toContain(INK);
  });

  it("StatusText colours only draft", () => {
    render(
      <>
        <StatusText status="draft" />
        <StatusText status="published" />
      </>,
    );
    expect(screen.getByText("draft").className).toContain(INK);
    expect(screen.getByText("published").className).not.toContain(INK);
  });
});
