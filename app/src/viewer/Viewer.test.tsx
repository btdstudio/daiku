import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { PLAN_PHASES, adr, fail, idea, ok, plan } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { ArtifactList } from "@/generated/ArtifactList";
import { Viewer } from "./Viewer";
import type { Selected } from "./selected";

const artifacts = (): ArtifactList => ({
  change: plan("1-a", { phase: "intake" }),
  phases: PLAN_PHASES,
  artifacts: [{ phase: "intake", path: "plan/00_intake/1.md" }],
});

function serve(read: unknown) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "artifacts") return ok(artifacts());
    if (cmd === "read_artifact") return read;
    if (cmd === "idea_list") return ok([idea(3, { title: "An idea", body: "idea body" })]);
    if (cmd === "adr_list") return ok([adr(7, { title: "An ADR", raw: "adr body" })]);
    return ok(null);
  });
}

function show(selected: Selected | null, onClose?: () => void) {
  return render(
    <QueryClientProvider client={makeQueryClient()}>
      <Viewer root="/w" selected={selected} focused={false} onFill={() => {}} onClose={onClose} />
    </QueryClientProvider>,
  );
}

describe("Viewer", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("is idle with nothing selected", () => {
    show(null);
    expect(screen.getByText("Open a file from the list")).toBeTruthy();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("routes an idea and an ADR to their panels", async () => {
    serve(ok(""));
    show({ kind: "idea", id: 3 });
    expect(await screen.findByText("idea body")).toBeTruthy();
    expect(screen.getByText("An idea")).toBeTruthy();
    cleanup();
    show({ kind: "adr", number: 7 });
    expect(await screen.findByText("adr body")).toBeTruthy();
    expect(screen.getByText("ADR-0007 An ADR")).toBeTruthy();
  });

  it("shows an escaped control character as text (AC3)", async () => {
    serve(ok("bell \\x1b[31m here"));
    show({ kind: "plan", name: "1-a" });
    expect(await screen.findByText(/\\x1b\[31m/)).toBeTruthy();
    expect(document.body.textContent).not.toContain("\u001b");
    expect(invoke).toHaveBeenCalledWith("read_artifact", {
      root: "/w",
      change: "1-a",
      path: "plan/00_intake/1.md",
    });
  });

  it("shows front matter in a pre above the body, not as markdown (AC4)", async () => {
    serve(ok("---\ntitle: x\n# not a heading\n---\n# Real"));
    const { container } = show({ kind: "plan", name: "1-a" });
    await screen.findByText("Real");
    const pre = container.querySelector("pre[data-front-matter]");
    expect(pre?.textContent).toBe("title: x\n# not a heading");
    expect(container.querySelectorAll("h1")).toHaveLength(1);
    expect(pre?.compareDocumentPosition(container.querySelector("h1") as Element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("shows a --- later in the file as a thematic break (AC4)", async () => {
    serve(ok("intro\n\n---\n\nafter"));
    const { container } = show({ kind: "plan", name: "1-a" });
    await screen.findByText("after");
    expect(container.querySelector("hr")).not.toBeNull();
    expect(container.querySelector("pre[data-front-matter]")).toBeNull();
  });

  it("shows the too-large message and no body (AC6)", async () => {
    serve(fail("TooLarge", "file too large to show (1.0 MiB)"));
    const { container } = show({ kind: "plan", name: "1-a" });
    expect(await screen.findByText("File too large to show (1.0 MiB)")).toBeTruthy();
    expect(container.querySelector("article")).toBeNull();
  });

  it("shows a failure's kind and message (AC7)", async () => {
    serve(fail("NotAnArtifact", "x isn't a file of 1-a"));
    show({ kind: "plan", name: "1-a" });
    expect(await screen.findByText(/NotAnArtifact: x isn't a file of 1-a/)).toBeTruthy();
  });

  it("offers a close button only while something is open", async () => {
    serve(ok("# A"));
    const onClose = vi.fn();
    const { rerender } = show({ kind: "plan", name: "1-a" }, onClose);
    fireEvent.click(await screen.findByRole("button", { name: "Close file" }));
    expect(onClose).toHaveBeenCalledOnce();
    rerender(
      <QueryClientProvider client={makeQueryClient()}>
        <Viewer root="/w" selected={null} focused={false} onFill={() => {}} onClose={onClose} />
      </QueryClientProvider>,
    );
    expect(screen.queryByRole("button", { name: "Close file" })).toBeNull();
  });
});
