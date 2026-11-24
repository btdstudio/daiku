import { QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useReducer, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { PLAN_PHASES, build, fail, ok, plan, status, ticket } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { ArtifactList } from "@/generated/ArtifactList";
import type { NextItem } from "@/generated/NextItem";
import { Frame } from "./Frame";
import type { Focus } from "./fold";
import { type FrameState, type Pane, type Tab, defaultView, reducer } from "./viewState";

const ROOT = "/w";
const tickets = [
  ticket("N-0020", { status: "draft", change: "", inbox: true, path: "build/tickets/N-0020.md" }),
  ticket("N-0021", { status: "ready", change: "", inbox: true, path: "build/tickets/N-0021.md" }),
];
const plans = [plan("4-plan", { phase: "spec" }), plan("5-other", { phase: "intake" })];
const artifacts: ArtifactList = {
  change: plan("4-plan", { phase: "spec" }),
  phases: PLAN_PHASES,
  artifacts: [
    { phase: "intake", path: "plan/00_intake/4.md" },
    { phase: "research", path: "plan/01_research/4/r.md" },
    { phase: "spec", path: "plan/specs/4-plan/spec.md" },
  ],
};

interface Opts {
  nextFails?: boolean;
  active?: boolean;
}
let opts: Opts = {};
let items: NextItem[] = [];

function serve(list: NextItem[], o: Opts = {}) {
  items = list;
  opts = o;
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    switch (cmd) {
      case "next":
        return opts.nextFails ? fail("Internal", "boom") : ok({ items });
      case "status": {
        const s = { ...status([]), plans, planPhases: PLAN_PHASES };
        return ok(
          opts.active
            ? {
                ...s,
                active: {
                  change: build("S-0023", { phase: "implement" }),
                  code: [],
                  initiative: "",
                  phases: [],
                },
              }
            : s,
        );
      }
      case "tickets_list":
        return ok({ tickets, unreadable: [] });
      case "artifacts":
        return ok(artifacts);
      case "read_artifact":
        return ok(`# Text of ${(args as { path: string }).path}`);
      case "idea_list":
      case "adr_list":
        return ok([]);
      default:
        return ok(null);
    }
  });
}

const buildItem = (ticketId = "N-0020"): NextItem => ({
  kind: "build",
  ticket: ticketId,
  title: "t",
  repo: "daiku",
  change: ticketId,
  since: null,
});
const planItem = (kind: "approve" | "start", phase = "spec", change = "4-plan"): NextItem => ({
  kind,
  change,
  loop: "plan",
  phase,
  active: false,
  since: null,
});

function setWidth(w: number) {
  Object.defineProperty(window, "innerWidth", { value: w, configurable: true, writable: true });
}

function Host({ tab, pane, focus = "split" }: { tab: Tab; pane: Pane; focus?: Focus }) {
  const init: FrameState = {
    view: { ...defaultView, open: [ROOT], selected: ROOT, tab, pane, focus },
    refs: { [ROOT]: { root: ROOT, name: "ws", shown: "/w", isWorkspace: true } },
    notWorkspace: null,
    changes: 0,
  };
  const [state, dispatch] = useReducer(reducer, init);
  const [client] = useState(makeQueryClient);
  return (
    <QueryClientProvider client={client}>
      <Frame state={state} dispatch={dispatch} />
      <output data-testid="view">{`${state.view.tab}/${state.view.pane}/${state.view.focus}`}</output>
    </QueryClientProvider>
  );
}

const view = () => screen.getByTestId("view").textContent;
const nextCalls = () => vi.mocked(invoke).mock.calls.filter(([c]) => c === "next").length;
const marked = () => [...document.querySelectorAll(".border-l-attention")];
const railCount = (label: string) =>
  document.querySelector(`[title="${label}"] .font-mono`) as HTMLElement;

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  setWidth(1500);
});
afterEach(cleanup);

describe("AC10: next stays current", () => {
  it("regaining window focus (visibilitychange, which TanStack listens for) refetches next", async () => {
    serve([buildItem()]);
    render(<Host tab="build" pane="viewer" />);
    await screen.findByRole("button", { name: /Next: N-0020/ });
    expect(nextCalls()).toBe(1);
    act(() => {
      window.dispatchEvent(new Event("visibilitychange"));
    });
    await waitFor(() => expect(nextCalls()).toBe(2));
  });

  it("a failed refetch keeps the last item's text", async () => {
    serve([buildItem()]);
    render(<Host tab="build" pane="viewer" />);
    const text = "Next: N-0020 is a draft. Start its build when you're ready.";
    await screen.findByRole("button", { name: text });
    serve([buildItem()], { nextFails: true });
    act(() => {
      window.dispatchEvent(new Event("visibilitychange"));
    });
    await waitFor(() => expect(nextCalls()).toBe(2));
    // Let the failure settle, then the text must still be there.
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.getByRole("button", { name: text })).toBeTruthy();
  });
});

describe("AC8: clicks", () => {
  it("a plan start item goes to the Plan tab with that plan and its phase", async () => {
    serve([planItem("start", "research")]);
    render(<Host tab="build" pane="viewer" />);
    fireEvent.click(await screen.findByRole("button", { name: "Next: start research for 4-plan" }));
    expect(view()).toBe("plan/viewer/split");
    expect(await screen.findByText("Text of plan/01_research/4/r.md")).toBeTruthy();
    expect(screen.getByRole("button", { name: /^research/ }).getAttribute("aria-pressed")).toBe(
      "true",
    );
  });

  it("an unknown item kind shows its text with no button", async () => {
    serve([{ kind: "mystery" } as unknown as NextItem]);
    render(<Host tab="plan" pane="list" />);
    expect(await screen.findByText("Next: something needs you")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /something needs you/ })).toBeNull();
    expect(view()).toBe("plan/list/split");
  });

  it("clicking again after choosing another phase returns to the nudge's phase", async () => {
    serve([planItem("start", "research")]);
    render(<Host tab="plan" pane="viewer" />);
    const click = async () =>
      fireEvent.click(await screen.findByRole("button", { name: "Next: start research for 4-plan" }));
    const pressed = (name: RegExp) =>
      screen.getByRole("button", { name }).getAttribute("aria-pressed");
    await click();
    await screen.findByText("Text of plan/01_research/4/r.md");
    expect(pressed(/^research/)).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: /^intake/ }));
    expect(pressed(/^intake/)).toBe("true");
    expect(pressed(/^research/)).toBe("false");
    await click();
    await waitFor(() => expect(pressed(/^research/)).toBe("true"));
  });
});

describe("AC1/AC2 through Frame: marks", () => {
  it("a build item marks its Build list row, and the folded rail's Draft count is amber", async () => {
    serve([buildItem("N-0020")]);
    const { unmount } = render(<Host tab="build" pane="viewer" />);
    await screen.findByRole("button", { name: /Next: N-0020/ });
    await waitFor(() => expect(marked().length).toBe(1));
    expect(marked()[0].textContent).toContain("N-0020");
    unmount();

    setWidth(1200);
    render(<Host tab="build" pane="viewer" />);
    await screen.findByRole("button", { name: /Next: N-0020/ });
    await waitFor(() => expect(railCount("Draft").className).toContain("text-attention-ink"));
    const amber = [...document.querySelectorAll("[title] > .font-mono.text-attention-ink")];
    expect(amber).toEqual([railCount("Draft")]);
    expect(amber[0].textContent).toBe("1");
  });

  it("a build item marks its board card", async () => {
    serve([buildItem("N-0021")]);
    render(<Host tab="build" pane="viewer" focus="board" />);
    await screen.findByRole("button", { name: /Next: N-0021/ });
    await waitFor(() => expect(marked().length).toBe(1));
    expect(marked()[0].textContent).toContain("N-0021");
  });

  it("a plan approve item marks its Plan list row, and the folded rail's Plans count is amber", async () => {
    serve([planItem("approve")]);
    const { unmount } = render(<Host tab="plan" pane="viewer" />);
    await screen.findByRole("button", { name: /Next: 4-plan/ });
    await waitFor(() => expect(marked().length).toBe(1));
    expect(marked()[0].textContent).toContain("4-plan");
    unmount();

    setWidth(1200);
    render(<Host tab="plan" pane="viewer" />);
    await screen.findByRole("button", { name: /Next: 4-plan/ });
    await waitFor(() => expect(railCount("Plans").className).toContain("text-attention-ink"));
    expect(railCount("Ideas").className).not.toContain("text-attention-ink");
  });

  it("a plan item marks its Plan board card", async () => {
    serve([planItem("start", "research", "5-other")]);
    render(<Host tab="plan" pane="viewer" focus="board" />);
    await screen.findByRole("button", { name: /Next: start research for 5-other/ });
    await waitFor(() => expect(marked().length).toBe(1));
    expect(marked()[0].textContent).toContain("5-other");
  });
});

describe("AC3 through Frame: nothing waiting", () => {
  it.each([
    ["build", "split", 1500],
    ["build", "board", 1500],
    ["build", "split", 1200],
    ["plan", "split", 1500],
    ["plan", "board", 1500],
    ["plan", "split", 1200],
  ] as [Tab, Focus, number][])("%s tab, %s focus, %ipx: no mark and no amber count", async (tab, focus, w) => {
    setWidth(w);
    serve([]);
    render(<Host tab={tab} pane="viewer" focus={focus} />);
    await screen.findByText("Nothing is waiting on you");
    // Wait for the data the marks would be drawn from.
    await waitFor(() =>
      expect(
        vi.mocked(invoke).mock.calls.some(([c]) => c === "tickets_list" || c === "status"),
      ).toBe(true),
    );
    await new Promise((r) => setTimeout(r, 30));
    expect(document.querySelectorAll(".border-l-attention").length).toBe(0);
    expect(document.querySelectorAll("[title] > .font-mono.text-attention-ink").length).toBe(0);
  });
});

describe("AC7: the narrow fold", () => {
  it("shows the active change and phase, with the name-only tab rule", async () => {
    setWidth(800);
    serve([], { active: true });
    render(<Host tab="plan" pane="list" />);
    await screen.findByText("S-0023");
    const bar = document.querySelector("footer") as HTMLElement;
    expect(bar.textContent).toContain("ws · S-0023 implement");
    expect(bar.textContent).not.toContain("Plan");
  });
});
