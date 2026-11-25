import { describe, expect, it } from "vitest";
import { chipLabels } from "./chips";

describe("chipLabels", () => {
  it("uses the first letter when names differ", () => {
    expect(chipLabels(["alpha", "beta"])).toEqual(["A", "B"]);
  });
  it("tells daiku and daiku-next apart by initials", () => {
    expect(chipLabels(["daiku", "daiku-next"])).toEqual(["D", "DN"]);
  });
  it("tells dart and daiku-next apart", () => {
    expect(chipLabels(["dart", "daiku-next"])).toEqual(["D", "DN"]);
  });
  it("falls back to a position when initials and first two letters collide", () => {
    expect(chipLabels(["daiku", "dash"])).toEqual(["D1", "D2"]);
  });
  it("skips already used labels in numbered fallbacks", () => {
    expect(chipLabels(["dash", "daiku", "d1x"])).toEqual(["D2", "D3", "D1"]);
  });
  it("numbers three identical names", () => {
    expect(chipLabels(["daiku", "daiku", "daiku"])).toEqual(["D1", "D2", "D3"]);
  });
  it("keeps a non-letter first character as is", () => {
    expect(chipLabels(["9lives", "beta"])).toEqual(["9", "B"]);
  });
  it("is pairwise distinct for fixed sets", () => {
    const sets = [
      ["daiku", "daiku-next", "dash", "dart"],
      ["a", "A", "a-b", "a_b", "ab"],
      ["x", "x", "xy", "x-y", "xz"],
      ["daiku", "daiku-next", "daiku-nest", "dn"],
    ];
    for (const names of sets) {
      const labels = chipLabels(names);
      expect(labels).toHaveLength(names.length);
      expect(new Set(labels).size).toBe(labels.length);
    }
  });
});
