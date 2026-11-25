import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { build, status, ticket } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { NextItem } from "@/generated/NextItem";
import { StatusBar } from "./StatusBar";
import { useNudge } from "./useNudge";

afterEach(cleanup);

describe("StatusBar", () => {
  it("shows the active change and its phase beside the workspace and tab", () => {
    const { container } = render(
      <StatusBar
        fold="wide"
        name="ws"
        tab="build"
        active={{ name: "S-0023", phase: "implement" }}
        nudge={null}
      />,
    );
    expect(container.textContent).toBe("ws · Build · S-0023 implement");
  });

  it("keeps the name-only tab rule below wide, and shows the active change", () => {
    const { container } = render(
      <StatusBar fold="folded" name="ws" tab="plan" active={{ name: "4-x", phase: "spec" }} nudge={null} />,
    );
    expect(container.textContent).toBe("ws · 4-x spec");
  });

  it("shows nothing extra without an active change", () => {
    const { container } = render(<StatusBar fold="wide" name="ws" tab="plan" nudge={null} />);
    expect(container.textContent).toBe("ws · Plan");
    expect(container.querySelector('[data-slot="active"]')).toBeNull();
  });

  it("renders the nudge as an amber button with its text as the title", () => {
    const onNudge = vi.fn();
    render(<StatusBar fold="wide" name="ws" tab="build" nudge="Next: N-1 is a draft." onNudge={onNudge} />);
    const b = screen.getByRole("button", { name: "Next: N-1 is a draft." });
    expect(b.getAttribute("title")).toBe("Next: N-1 is a draft.");
    expect(b.className).toContain("text-attention-ink");
    expect(b.className).toContain("truncate");
    fireEvent.click(b);
    expect(onNudge).toHaveBeenCalledOnce();
  });

  it("shows text without a button when it is not clickable, and nothing for null", () => {
    const { rerender } = render(
      <StatusBar fold="wide" name="ws" tab="build" nudge="Nothing is waiting on you" />,
    );
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("Nothing is waiting on you")).toBeTruthy();
    rerender(<StatusBar fold="wide" name="ws" tab="build" nudge={null} />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(document.querySelector('[data-slot="nudge"]')).toBeNull();
  });

  it("renders a control-character id as text, with no HTML", () => {
    const text = "Next: <img src=x onerror=1> \\x1b[0m";
    const { container } = render(
      <StatusBar fold="wide" name="ws" tab="build" nudge={text} onNudge={() => {}} />,
    );
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByRole("button").textContent).toBe(text);
  });

  it("shows the warnings on one line, beside the active change and the nudge", () => {
    const { container } = render(
      <StatusBar
        fold="wide"
        name="ws"
        tab="build"
        active={{ name: "S-0019", phase: "implement" }}
        warnings={["status warning", "next warning"]}
        nudge="Next: N-1 is a draft."
        onNudge={() => {}}
      />,
    );
    const feedback = container.querySelector('[data-slot="feedback"]') as HTMLElement;
    expect(feedback.textContent).toBe("status warning · next warning");
    expect(feedback.className).toContain("truncate");
    expect(container.querySelectorAll("p")).toHaveLength(0);
    expect(screen.queryByRole("alert")).toBeNull();
    const bar = container.querySelector("footer") as HTMLElement;
    expect(bar.className).toContain("h-[30px]");
    expect(bar.textContent).toContain("ws · Build · S-0019 implement");
    expect(screen.getByRole("button", { name: "Next: N-1 is a draft." })).toBeTruthy();
  });

  it("shows a next failure beside the kept nudge, with the kind visible before long warnings", () => {
    const onNudge = vi.fn();
    const { container } = render(
      <StatusBar
        fold="wide"
        name="ws"
        tab="build"
        warnings={["a long warning that should truncate before it hides the failure"]}
        error={new Error("Io: disk went away")}
        nudge="Next: N-1 is a draft."
        onNudge={onNudge}
      />,
    );
    const feedback = container.querySelector('[data-slot="feedback"]') as HTMLElement;
    expect(feedback.textContent).toBe(
      "Error: Io: disk went away · a long warning that should truncate before it hides the failure",
    );
    expect(feedback.className).toContain("truncate");
    expect((container.querySelector("footer") as HTMLElement).className).toContain("h-[30px]");
    fireEvent.click(screen.getByRole("button", { name: "Next: N-1 is a draft." }));
    expect(onNudge).toHaveBeenCalledOnce();
  });

  it("adds nothing for no warnings, keeping the 30px bar and its left text", () => {
    const { container } = render(<StatusBar fold="folded" name="ws" tab="plan" nudge={null} />);
    expect(container.querySelector('[data-slot="feedback"]')).toBeNull();
    expect((container.querySelector("footer") as HTMLElement).className).toContain("h-[30px]");
    expect(container.textContent).toBe("ws");
  });
});

/** A `build` item for N-0020, the nudge's first. */
const item: NextItem = {
  kind: "build",
  ticket: "N-0020",
  title: "t",
  repo: "daiku",
  change: "N-0020",
  since: null,
};

type NextReply =
  | { ok: true; data: { items: NextItem[] }; warnings?: string[] }
  | { ok: false; error: { kind: string; message: string } };

/** Core's replies, each with the warnings/failure the test asks for. */
function serve(next: NextReply, warnings: { tickets?: string[] } = {}) {
  const replies: Record<string, unknown> = {
    next:
      next.ok
        ? { schema: 1, data: next.data, warnings: next.warnings ?? [] }
        : { schema: 1, error: next.error },
    tickets_list: {
      schema: 1,
      data: { tickets: [ticket("N-0020", { status: "draft" })], unreadable: [] },
      warnings: warnings.tickets ?? [],
    },
    status: {
      schema: 1,
      data: status([{ repo: "daiku", builds: [build("N-0020")] }]),
      warnings: [],
    },
  };
  vi.mocked(invoke).mockImplementation(async (cmd: string) =>
    cmd in replies ? replies[cmd] : { schema: 1, data: null, warnings: [] },
  );
}

function nudged() {
  const client = makeQueryClient();
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(() => useNudge("/w", "wide"), { wrapper: Wrapper });
}

describe("useNudge warnings and errors", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("exposes next's then tickets' warnings, leaving the item, marks and target alone", async () => {
    serve({ ok: true, data: { items: [item] }, warnings: ["next warning"] }, { tickets: ["tickets warning"] });
    const { result } = nudged();

    await waitFor(() =>
      expect(result.current.text).toBe(
        "Next: N-0020 is a draft. Start its build when you're ready.",
      ),
    );
    expect(result.current.warnings).toEqual(["next warning", "tickets warning"]);
    expect(result.current.error).toBeNull();
    expect(result.current.item).toEqual(item);
    expect(result.current.nextTicket).toBe("N-0020");
    expect(result.current.target(result.current.item)).toEqual({
      tab: "build",
      sel: { kind: "ticket", key: "plan/specs/4-plan/tickets/N-0020.md" },
    });
  });

  it("has no warnings when both queries carry none", async () => {
    serve({ ok: true, data: { items: [] } });
    const { result } = nudged();

    await waitFor(() => expect(result.current.text).toBe("Nothing is waiting on you"));
    expect(result.current.warnings).toEqual([]);
    expect(result.current.error).toBeNull();
    expect(result.current.item).toBeUndefined();
  });

  it("keeps the last nudge when a later next read fails (AC1)", async () => {
    serve({ ok: true, data: { items: [item] } });
    const client = makeQueryClient();
    const Wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useNudge("/w", "wide"), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.nextTicket).toBe("N-0020"));

    serve({ ok: false, error: { kind: "Io", message: "disk went away" } });
    await client.invalidateQueries({ queryKey: ["/w", "next"] });
    await waitFor(() => expect(result.current.error).toBeTruthy());
    expect(result.current.text).toBe("Next: N-0020 is a draft. Start its build when you're ready.");
    expect(result.current.item).toEqual(item);
    expect(result.current.nextTicket).toBe("N-0020");
  });

  it("shows a failure when next has never loaded (AC2)", async () => {
    serve({ ok: false, error: { kind: "Io", message: "disk went away" } });
    const { result } = nudged();
    await waitFor(() => expect(result.current.error).toBeTruthy());
    expect(result.current.text).toBeNull();
    expect(result.current.item).toBeUndefined();
  });

  it("clears the failure when next succeeds again (AC3)", async () => {
    serve({ ok: false, error: { kind: "Io", message: "disk went away" } });
    const client = makeQueryClient();
    const Wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useNudge("/w", "wide"), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.error).toBeTruthy());

    serve({ ok: true, data: { items: [item] } });
    await client.invalidateQueries({ queryKey: ["/w", "next"] });
    await waitFor(() => expect(result.current.error).toBeNull());
    expect(result.current.text).toBe("Next: N-0020 is a draft. Start its build when you're ready.");
  });
});
