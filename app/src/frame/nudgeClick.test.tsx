import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useReducer, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { PLAN_PHASES, ok, plan, status, ticket } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { ArtifactList } from "@/generated/ArtifactList";
import type { NextItem } from "@/generated/NextItem";
import { Frame } from "./Frame";
import type { Focus } from "./fold";
import { type FrameState, type Pane, type Tab, defaultView, reducer } from "./viewState";

const ROOT = "/w";
const tickets = [ticket("N-0020", { status: "draft", change: "", inbox: true, path: "build/tickets/N-0020.md" })];
const artifacts: ArtifactList = {
  change: plan("4-plan", { phase: "intake" }),
  phases: PLAN_PHASES,
  artifacts: [
    { phase: "intake", path: "plan/00_intake/4.md" },
    { phase: "spec", path: "plan/specs/4-plan/spec.md" },
  ],
};

function serve(items: NextItem[]) {
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    switch (cmd) {
      case "next":
        return ok({ items });
      case "status":
        return ok({ ...status([]), plans: [plan("4-plan")], planPhases: PLAN_PHASES });
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

const approve = (over: Partial<Extract<NextItem, { kind: "approve" }>> = {}): NextItem => ({
  kind: "approve",
  change: "4-plan",
  loop: "plan",
  phase: "spec",
  active: false,
  since: null,
  ...over,
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
const nudge = async (text: RegExp | string) => screen.findByRole("button", { name: text });

describe("clicking the nudge", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    setWidth(1500);
  });
  afterEach(cleanup);

  it("build: goes to the Build tab with the ticket open", async () => {
    serve([{ kind: "build", ticket: "N-0020", title: "t", repo: "daiku", change: "N-0020", since: null }]);
    render(<Host tab="plan" pane="viewer" />);
    const b = await nudge("Next: N-0020 is a draft. Start its build when you're ready.");
    fireEvent.click(b);
    expect(view()).toBe("build/viewer/split");
    expect(await screen.findByText("N-0020 · Title N-0020")).toBeTruthy();
  });

  it("plan approve: goes to the Plan tab, the plan open on that phase", async () => {
    serve([approve()]);
    render(<Host tab="build" pane="viewer" />);
    fireEvent.click(await nudge("Next: 4-plan spec is waiting for your approval"));
    expect(view()).toBe("plan/viewer/split");
    expect(await screen.findByText("Text of plan/specs/4-plan/spec.md")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("read_artifact", {
      root: ROOT,
      change: "4-plan",
      path: "plan/specs/4-plan/spec.md",
    });
    const chip = screen.getByRole("button", { name: /^spec/ });
    expect(chip.getAttribute("aria-pressed")).toBe("true");
  });

  it("build-loop start: goes to that build's ticket on the Build tab", async () => {
    serve([approve({ kind: "start", loop: "build", change: "N-0020", phase: "implement" } as never)]);
    render(<Host tab="plan" pane="viewer" />);
    fireEvent.click(await nudge("Next: start implement for N-0020"));
    expect(view()).toBe("build/viewer/split");
    expect(await screen.findByText("N-0020 · Title N-0020")).toBeTruthy();
  });

  it("narrow fold: also switches to the viewer pane, in the short form", async () => {
    setWidth(800);
    serve([approve()]);
    render(<Host tab="build" pane="list" />);
    fireEvent.click(await nudge("Next: 4-plan · spec"));
    expect(view()).toBe("plan/viewer/split");
  });

  it("board focus: returns to the split so the viewer shows", async () => {
    serve([approve()]);
    render(<Host tab="build" pane="viewer" focus="board" />);
    fireEvent.click(await nudge("Next: 4-plan spec is waiting for your approval"));
    expect(view()).toBe("plan/viewer/split");
  });

  it("no matching ticket: shows the text with no button", async () => {
    serve([{ kind: "build", ticket: "Z-9", title: "t", repo: "daiku", change: "Z-9", since: null }]);
    render(<Host tab="plan" pane="list" />);
    expect(await screen.findByText("Next: Z-9. Start its build when you're ready.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Next: Z-9/ })).toBeNull();
    expect(view()).toBe("plan/list/split");
    expect(screen.queryByText(/Title/)).toBeNull();
  });

  it("an empty list shows the text with no button", async () => {
    serve([]);
    render(<Host tab="build" pane="viewer" />);
    expect(await screen.findByText("Nothing is waiting on you")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("button", { name: /Nothing/ })).toBeNull());
  });
});
