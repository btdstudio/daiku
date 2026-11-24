import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { Viewer } from "./Viewer";

const ok = (data: string) => ({ schema: 1, data, warnings: [] });
const fail = (kind: string, message: string) => ({ schema: 1, error: { kind, message } });

function show(reply: unknown, path = "plan/a.md") {
  vi.mocked(invoke).mockResolvedValue(reply);
  return render(<Viewer root="/w" path={path} focused={false} onFill={() => {}} />);
}

describe("Viewer", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows an escaped control character as text (AC3)", async () => {
    show(ok("bell \\x1b[31m here"));
    expect(await screen.findByText(/\\x1b\[31m/)).toBeTruthy();
    expect(document.body.textContent).not.toContain("\u001b");
    expect(invoke).toHaveBeenCalledWith("read_text", { root: "/w", rel: "plan/a.md" });
  });

  it("shows front matter in a pre above the body, not as markdown (AC4)", async () => {
    const { container } = show(ok("---\ntitle: x\n# not a heading\n---\n# Real"));
    await screen.findByText("Real");
    const pre = container.querySelector("pre[data-front-matter]");
    expect(pre?.textContent).toBe("title: x\n# not a heading");
    expect(container.querySelectorAll("h1")).toHaveLength(1);
    expect(pre?.compareDocumentPosition(container.querySelector("h1") as Element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("shows a --- later in the file as a thematic break (AC4)", async () => {
    const { container } = show(ok("intro\n\n---\n\nafter"));
    await screen.findByText("after");
    expect(container.querySelector("hr")).not.toBeNull();
    expect(container.querySelector("pre")).toBeNull();
  });

  it("shows the too-large message and no body (AC6)", async () => {
    const { container } = show(fail("TooLarge", "file too large to show (1.0 MiB)"));
    expect(await screen.findByText("File too large to show (1.0 MiB)")).toBeTruthy();
    expect(container.querySelector("article")).toBeNull();
  });

  it("shows a failure's message (AC7)", async () => {
    show(fail("ReadOutside", "read ../outside: path is outside the workspace"), "../outside");
    expect(await screen.findByText(/path is outside the workspace/)).toBeTruthy();
  });

  it("keeps no stale file when the path changes", async () => {
    const { rerender } = show(ok("first"));
    await screen.findByText("first");
    let resolve: (v: unknown) => void = () => {};
    vi.mocked(invoke).mockReturnValue(new Promise((r) => (resolve = r)));
    rerender(<Viewer root="/w" path="b.md" focused={false} onFill={() => {}} />);
    expect(screen.queryByText("first")).toBeNull();
    expect(screen.getByText("Reading…")).toBeTruthy();
    resolve(ok("second"));
    expect(await screen.findByText("second")).toBeTruthy();
  });
  it("offers a close button only while a file is open", async () => {
    vi.mocked(invoke).mockResolvedValue(ok("# A"));
    const onClose = vi.fn();
    const { rerender } = render(
      <Viewer root="/w" path="a.md" focused={false} onFill={() => {}} onClose={onClose} />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Close file" }));
    expect(onClose).toHaveBeenCalledOnce();
    rerender(<Viewer root="/w" path={null} focused={false} onFill={() => {}} onClose={onClose} />);
    expect(screen.queryByRole("button", { name: "Close file" })).toBeNull();
  });
});
