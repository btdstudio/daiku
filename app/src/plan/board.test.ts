import { describe, expect, it } from "vitest";
import { PLAN_PHASES, plan } from "@/board/fixtures";
import { planColumn } from "./board";

describe("planColumn", () => {
  it("is the plan's phase", () => {
    expect(planColumn(plan("1-a", { phase: "spec" }), PLAN_PHASES)).toBe("spec");
  });
  it("is the first phase when the phase is empty", () => {
    expect(planColumn(plan("1-a", { phase: "" }), PLAN_PHASES)).toBe("intake");
  });
});
