import { describe, expect, it } from "vitest";
import { type Envelope, isFailure } from "./envelope";

describe("isFailure", () => {
  it("is false for a Success", () => {
    const e: Envelope<{ n: number }> = { schema: 1, data: { n: 1 }, warnings: [] };
    expect(isFailure(e)).toBe(false);
  });

  it("is true for a Failure", () => {
    const e: Envelope<{ n: number }> = {
      schema: 1,
      error: { kind: "NotAWorkspace", message: "not a workspace" },
    };
    expect(isFailure(e)).toBe(true);
  });
});
