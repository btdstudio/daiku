import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { PLAN_PHASES, fail, ok, plan, planStatus } from "./fixtures";
import { makeQueryClient } from "@/data/queryClient";
import { PlanBoard } from "./PlanBoard";

function show(onSelect = vi.fn()) {
  const client = makeQueryClient();
  render(
    <QueryClientProvider client={client}>
      <PlanBoard root="/w" onSelect={onSelect} />
    </QueryClientProvider>,
  );
  return { client, onSelect };
}
const col = (name: string) => screen.getByRole("group", { name });

describe("PlanBoard", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("has one column per planPhases entry, each plan in its phase's column", async () => {
    vi.mocked(invoke).mockResolvedValue(
      ok(
        planStatus([
          plan("1-a", { phase: "spec", gates: { spec: "draft" }, repos: ["daiku", "api"] }),
          plan("2-b", { phase: "" }),
          plan("3-c", { phase: "spec", gates: { spec: "approved" } }),
        ]),
      ),
    );
    show();
    await screen.findByText("1-a");
    expect(screen.getAllByRole("group").map((g) => g.getAttribute("aria-label"))).toEqual(PLAN_PHASES);
    expect(within(col("spec")).getAllByRole("button").map((b) => b.textContent)).toEqual([
      "1-adaiku, apidraft2026-01-01T00:00:00.000Z",
      "3-capproved2026-01-01T00:00:00.000Z",
    ]);
    // An empty phase puts the plan in the first column, "not started".
    expect(within(col("intake")).getByText("2-b")).toBeTruthy();
    expect(within(col("intake")).getByText("not started")).toBeTruthy();
    expect(within(col("research")).queryAllByRole("button")).toHaveLength(0);
  });

  it("selects the plan by name on a click", async () => {
    vi.mocked(invoke).mockResolvedValue(ok(planStatus([plan("1-a", { phase: "explore" })])));
    const { onSelect } = show();
    fireEvent.click(await screen.findByText("1-a"));
    expect(onSelect).toHaveBeenCalledWith("1-a");
  });

  it("says there are no open plans", async () => {
    vi.mocked(invoke).mockResolvedValue(ok(planStatus([])));
    show();
    expect(await screen.findByText("No open plans")).toBeTruthy();
  });

  it("keeps the last cards under an error after a failed refetch", async () => {
    vi.mocked(invoke).mockResolvedValue(ok(planStatus([plan("1-a")])));
    const { client } = show();
    await screen.findByText("1-a");
    vi.mocked(invoke).mockResolvedValue(fail("Io", "disk went away"));
    await client.refetchQueries();
    expect((await screen.findByRole("alert")).textContent).toContain("disk went away");
    expect(screen.getByText("1-a")).toBeTruthy();
  });
});
