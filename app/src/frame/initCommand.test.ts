import { describe, expect, it } from "vitest";
import { initCommand } from "./initCommand";

describe("initCommand", () => {
  it("quotes a plain path", () => {
    expect(initCommand("/tmp/work")).toBe("daiku init '/tmp/work'");
  });
  it("keeps a space inside the quotes", () => {
    expect(initCommand("/tmp/my work")).toBe("daiku init '/tmp/my work'");
  });
  it("escapes a single quote", () => {
    expect(initCommand("/tmp/it's")).toBe("daiku init '/tmp/it'\\''s'");
  });
});
