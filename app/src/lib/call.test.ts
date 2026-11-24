import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { call } from "./call";

describe("call", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("resolves with the envelope the command returns", async () => {
    const e = { schema: 1, data: { n: 1 }, warnings: [] };
    vi.mocked(invoke).mockResolvedValue(e);
    await expect(call("status", { root: "/w" })).resolves.toEqual(e);
    expect(invoke).toHaveBeenCalledWith("status", { root: "/w" });
  });

  it("turns a rejection into an Internal Failure", async () => {
    vi.mocked(invoke).mockRejectedValue("command status not found");
    await expect(call("status")).resolves.toEqual({
      schema: 1,
      error: { kind: "Internal", message: "command status not found" },
    });
  });
});
