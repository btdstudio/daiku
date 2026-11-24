import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { BuildBoard } from "@/board/BuildBoard";
import { PlanBoard } from "@/board/PlanBoard";
import { adr, idea, ok, plan, status, ticket } from "@/board/fixtures";
import { BuildList } from "@/build/BuildList";
import { makeQueryClient } from "@/data/queryClient";
import { PlanList } from "@/plan/PlanList";
import { ListRail } from "./ListRail";

const tickets = [ticket("S-1", { status: "draft" }), ticket("S-2", { status: "ready" })];
const plans = [plan("1-a", { phase: "spec" }), plan("2-b", { phase: "intake" })];

function serve() {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "status") return ok({ ...status([]), plans, planPhases: ["intake", "spec"] });
    if (cmd === "tickets_list") return ok({ tickets, unreadable: [] });
    if (cmd === "idea_list") return ok([idea(1)]);
    return ok([adr(1)]);
  });
}
const wrap = (ui: ReactNode) => (
  <QueryClientProvider client={makeQueryClient()}>{ui}</QueryClientProvider>
);
const marked = (c: HTMLElement) => c.querySelectorAll(".border-l-attention");

describe("next marks", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    serve();
  });
  afterEach(cleanup);

  it("Build list: one row for nextId, none without", async () => {
    const { container, rerender } = render(
      wrap(<BuildList root="/w" onFill={() => {}} onSelect={() => {}} nextId="S-2" />),
    );
    await screen.findByText("S-2");
    expect(marked(container).length).toBe(1);
    expect(marked(container)[0].textContent).toContain("S-2");
    rerender(wrap(<BuildList root="/w" onFill={() => {}} onSelect={() => {}} />));
    expect(marked(container).length).toBe(0);
  });

  it("Build board: one card for nextId, none without", async () => {
    const { container, rerender } = render(
      wrap(<BuildBoard root="/w" onSelect={() => {}} nextId="S-1" />),
    );
    await screen.findByText("S-1");
    expect(marked(container).length).toBe(1);
    expect(marked(container)[0].textContent).toContain("S-1");
    rerender(wrap(<BuildBoard root="/w" onSelect={() => {}} />));
    expect(marked(container).length).toBe(0);
  });

  it("Plan list: one row for nextPlan, none without", async () => {
    const { container, rerender } = render(
      wrap(<PlanList root="/w" onFill={() => {}} onSelect={() => {}} nextPlan="2-b" />),
    );
    await screen.findByText("2-b");
    expect(marked(container).length).toBe(1);
    expect(marked(container)[0].textContent).toContain("2-b");
    rerender(wrap(<PlanList root="/w" onFill={() => {}} onSelect={() => {}} />));
    expect(marked(container).length).toBe(0);
  });

  it("Plan board: one card for nextPlan, none without", async () => {
    const { container, rerender } = render(
      wrap(<PlanBoard root="/w" onSelect={() => {}} nextPlan="1-a" />),
    );
    await screen.findByText("1-a");
    expect(marked(container).length).toBe(1);
    expect(marked(container)[0].textContent).toContain("1-a");
    rerender(wrap(<PlanBoard root="/w" onSelect={() => {}} />));
    expect(marked(container).length).toBe(0);
  });
});

describe("ListRail amber count", () => {
  afterEach(cleanup);
  const counts = { inBuild: 1, ready: 2, draft: 3, done: 4 };
  const amber = (c: HTMLElement) =>
    [...c.querySelectorAll(".text-attention-ink")].map((e) => e.textContent);

  it("colours only nextGroup's count", () => {
    const { container } = render(
      <ListRail tab="build" counts={counts} nextGroup="ready">
        <p />
      </ListRail>,
    );
    expect(amber(container)).toEqual(["2"]);
  });

  it("colours none without a nextGroup, or for an unknown one", () => {
    const { container, rerender } = render(
      <ListRail tab="build" counts={counts}>
        <p />
      </ListRail>,
    );
    expect(amber(container)).toEqual([]);
    rerender(
      <ListRail tab="plan" counts={{ Plans: 5 }} nextGroup="Plans">
        <p />
      </ListRail>,
    );
    expect(amber(container)).toEqual(["5"]);
  });
});
