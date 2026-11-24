import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { adr, idea, ok } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import { AdrPanel } from "./AdrPanel";
import { IdeaPanel } from "./IdeaPanel";

// What the envelope delivers: control characters already escaped as text.
const HOSTILE = "before <script>window.__ran = true</script> bell \\x1b[31m after\n\n<img src=x onerror=alert(1)>";

function wrap(ui: React.ReactNode) {
  return <QueryClientProvider client={makeQueryClient()}>{ui}</QueryClientProvider>;
}

describe("AdrPanel and IdeaPanel", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("renders an ADR's front matter and body, and the header title", async () => {
    vi.mocked(invoke).mockResolvedValue(
      ok([adr(7, { title: "Tauri 2", raw: "---\nstatus: accepted\n---\n\n# Context\n\ntext" })]),
    );
    const { container } = render(
      wrap(<AdrPanel root="/w" number={7} focused={false} onFill={() => {}} />),
    );
    await screen.findByText("Context");
    expect(screen.getByText("ADR-0007 Tauri 2")).toBeTruthy();
    expect(container.querySelector("pre[data-front-matter]")?.textContent).toBe("status: accepted");
  });

  it("renders no script from an ADR body and shows the escape as text (AC9)", async () => {
    vi.mocked(invoke).mockResolvedValue(ok([adr(7, { raw: HOSTILE })]));
    const { container } = render(
      wrap(<AdrPanel root="/w" number={7} focused={false} onFill={() => {}} />),
    );
    await screen.findByText(/\\x1b\[31m/);
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect((window as unknown as { __ran?: boolean }).__ran).toBeUndefined();
    expect(document.body.textContent).not.toContain("\u001b");
  });

  it("renders no script from an idea body and shows the escape as text (AC9)", async () => {
    vi.mocked(invoke).mockResolvedValue(ok([idea(3, { body: HOSTILE })]));
    const { container } = render(
      wrap(<IdeaPanel root="/w" id={3} focused={false} onFill={() => {}} />),
    );
    await screen.findByText(/\\x1b\[31m/);
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect((window as unknown as { __ran?: boolean }).__ran).toBeUndefined();
  });

  it("shows an idea's dl, title, and omits empty fields", async () => {
    vi.mocked(invoke).mockResolvedValue(
      ok([idea(3, { title: "App UI", status: "parked", reason: "later", changes: ["4-app", "5-x"], body: "# Body" })]),
    );
    const { container } = render(
      wrap(<IdeaPanel root="/w" id={3} focused={false} onFill={() => {}} />),
    );
    await screen.findByText("Body");
    expect(screen.getByText("App UI")).toBeTruthy();
    const dts = [...container.querySelectorAll("dt")].map((d) => d.textContent);
    expect(dts).toEqual(["status", "reason", "updated", "promoted"]);
    expect(screen.getByText("4-app, 5-x")).toBeTruthy();
  });

  it("says an idea or an ADR is gone", async () => {
    vi.mocked(invoke).mockResolvedValue(ok([]));
    render(wrap(<IdeaPanel root="/w" id={3} focused={false} onFill={() => {}} />));
    expect(await screen.findByText("This idea is no longer in the workspace")).toBeTruthy();
    cleanup();
    render(wrap(<AdrPanel root="/w" number={3} focused={false} onFill={() => {}} />));
    expect(await screen.findByText("This ADR is no longer in the workspace")).toBeTruthy();
  });
});
