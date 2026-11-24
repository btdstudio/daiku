import { describe, expect, it } from "vitest";
import { PLAN_PHASES, plan } from "@/board/fixtures";
import type { ArtifactList } from "@/generated/ArtifactList";
import { phaseStrip } from "./phaseStrip";

function list(over: Parameters<typeof plan>[1] = {}): ArtifactList {
  return {
    change: plan("4-plan", over),
    phases: PLAN_PHASES,
    artifacts: [
      { phase: "intake", path: "plan/00_intake/4.md" },
      { phase: "spec", path: "plan/specs/4-plan/functional-spec.md" },
      { phase: "spec", path: "plan/specs/4-plan/other.md" },
    ],
  };
}

describe("phaseStrip", () => {
  it("lists every phase in order with its gate status", () => {
    const strip = phaseStrip(
      list({ phase: "spec", gates: { intake: "approved", research: "skipped", spec: "draft" } }),
    );
    expect(strip.map((p) => p.name)).toEqual(PLAN_PHASES);
    expect(strip.map((p) => p.status)).toEqual([
      "approved",
      "skipped",
      "not started",
      "not started",
      "draft",
      "not started",
      "not started",
      "not started",
    ]);
  });

  it("marks a skipped phase with its reason", () => {
    const strip = phaseStrip(
      list({ gates: { research: "skipped" }, reasons: { research: "no vendors to compare" } }),
    );
    const research = strip.find((p) => p.name === "research");
    expect(research?.skipped).toBe(true);
    expect(research?.reason).toBe("no vendors to compare");
    expect(strip.filter((p) => p.skipped)).toHaveLength(1);
    expect(strip.find((p) => p.name === "intake")?.reason).toBe("");
  });

  it("takes the current phase from change.phase, or the first when it is empty", () => {
    const at = (phase: string) =>
      phaseStrip(list({ phase }))
        .filter((p) => p.current)
        .map((p) => p.name);
    expect(at("architecture")).toEqual(["architecture"]);
    expect(at("")).toEqual(["intake"]);
  });

  it("lists each phase's files in artifacts order, and none for an empty phase", () => {
    const strip = phaseStrip(list());
    expect(strip.find((p) => p.name === "spec")?.files).toEqual([
      "plan/specs/4-plan/functional-spec.md",
      "plan/specs/4-plan/other.md",
    ]);
    expect(strip.find((p) => p.name === "research")?.files).toEqual([]);
  });
});
