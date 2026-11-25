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

/** Core's replies, each with the warnings the test asks for. */
function serve(items: NextItem[], warnings: { next?: string[]; tickets?: string[] } = {}) {
  const replies: Record<string, unknown> = {
    next: { schema: 1, data: { items }, warnings: warnings.next ?? [] },
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

describe("useNudge warnings", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("exposes next's then tickets' warnings, leaving the item, marks and target alone", async () => {
    serve([item], { next: ["next warning"], tickets: ["tickets warning"] });
    const { result } = nudged();

    await waitFor(() =>
      expect(result.current.text).toBe(
        "Next: N-0020 is a draft. Start its build when you're ready.",
      ),
    );
    expect(result.current.warnings).toEqual(["next warning", "tickets warning"]);
    expect(result.current.item).toEqual(item);
    expect(result.current.nextTicket).toBe("N-0020");
    expect(result.current.target(result.current.item)).toEqual({
      tab: "build",
      sel: { kind: "ticket", key: "plan/specs/4-plan/tickets/N-0020.md" },
    });
  });

  it("has no warnings when both queries carry none", async () => {
    serve([]);
    const { result } = nudged();

    await waitFor(() => expect(result.current.text).toBe("Nothing is waiting on you"));
    expect(result.current.warnings).toEqual([]);
    expect(result.current.item).toBeUndefined();
  });
});
