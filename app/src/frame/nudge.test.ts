import { describe, expect, it } from "vitest";
import type { NextItem } from "@/generated/NextItem";
import { age, nudgeText } from "./nudge";

const NOW = Date.parse("2026-11-24T12:00:00Z");
const ago = (h: number) => new Date(NOW - h * 3_600_000).toISOString();

const build = (over: Partial<Extract<NextItem, { kind: "build" }>> = {}): NextItem => ({
  kind: "build",
  ticket: "N-0020",
  title: "t",
  repo: "daiku",
  change: "N-0020",
  since: null,
  ...over,
});
const approve = (since: string | null = null): NextItem => ({
  kind: "approve",
  change: "4-plan",
  loop: "plan",
  phase: "spec",
  active: false,
  since,
});
const start = (since: string | null = null): NextItem => ({
  kind: "start",
  change: "4-plan",
  loop: "plan",
  phase: "publish",
  active: false,
  since,
});

describe("age", () => {
  it("shows nothing up to 24h, hours to 48h, then days", () => {
    expect(age(ago(23), NOW)).toBe("");
    expect(age(ago(24), NOW)).toBe("");
    expect(age(ago(25), NOW)).toBe(" · 25h");
    expect(age(ago(49), NOW)).toBe(" · 2d");
    expect(age(ago(72), NOW)).toBe(" · 3d");
  });
  it("shows nothing for a missing or unparseable since", () => {
    expect(age(null, NOW)).toBe("");
    expect(age(undefined, NOW)).toBe("");
    expect(age("bogus", NOW)).toBe("");
  });
});

describe("nudgeText", () => {
  it("build, long and short, by ticket status", () => {
    expect(nudgeText(build(), "draft", "wide", NOW)).toBe(
      "Next: N-0020 is a draft. Start its build when you're ready.",
    );
    expect(nudgeText(build(), "ready", "folded", NOW)).toBe(
      "Next: N-0020 is ready. Start its build when you're ready.",
    );
    expect(nudgeText(build(), "draft", "narrow", NOW)).toBe("Next: N-0020 · draft");
    expect(nudgeText(build(), "ready", "narrow", NOW)).toBe("Next: N-0020 · ready");
  });
  it("build drops the status clause with no ticket status", () => {
    expect(nudgeText(build(), null, "wide", NOW)).toBe(
      "Next: N-0020. Start its build when you're ready.",
    );
    expect(nudgeText(build(), null, "narrow", NOW)).toBe("Next: N-0020");
  });
  it("approve and start, long and short", () => {
    expect(nudgeText(approve(), null, "wide", NOW)).toBe(
      "Next: 4-plan spec is waiting for your approval",
    );
    expect(nudgeText(approve(), null, "narrow", NOW)).toBe("Next: 4-plan · spec");
    expect(nudgeText(start(), null, "wide", NOW)).toBe("Next: start publish for 4-plan");
    expect(nudgeText(start(), null, "narrow", NOW)).toBe("Next: 4-plan · start publish");
  });
  it("appends the age in both forms", () => {
    expect(nudgeText(approve(ago(25)), null, "wide", NOW)).toBe(
      "Next: 4-plan spec is waiting for your approval · 25h",
    );
    expect(nudgeText(start(ago(72)), null, "narrow", NOW)).toBe(
      "Next: 4-plan · start publish · 3d",
    );
    expect(nudgeText(build({ since: ago(25) }), "draft", "narrow", NOW)).toBe(
      "Next: N-0020 · draft · 25h",
    );
    expect(nudgeText(approve(ago(23)), null, "wide", NOW)).not.toContain("·");
  });
  it("says nothing is waiting when the list is empty, and nothing with no data", () => {
    expect(nudgeText(undefined, null, "wide", NOW)).toBe("Nothing is waiting on you");
    expect(nudgeText(undefined, null, "narrow", NOW)).toBe("Nothing is waiting on you");
    expect(nudgeText(null, null, "wide", NOW)).toBeNull();
  });
  it("handles an unknown kind", () => {
    const odd = { kind: "wat" } as unknown as NextItem;
    expect(nudgeText(odd, null, "wide", NOW)).toBe("Next: something needs you");
    expect(nudgeText(odd, null, "narrow", NOW)).toBe("Next: something needs you");
  });
  it("passes a control-character id through unchanged", () => {
    expect(nudgeText(build({ ticket: "N-\\x1b[0m" }), null, "narrow", NOW)).toBe("Next: N-\\x1b[0m");
    expect(nudgeText(build({ ticket: "N-\x1b" }), null, "narrow", NOW)).toBe("Next: N-\x1b");
  });
});
