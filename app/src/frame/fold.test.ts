import { describe, expect, it } from "vitest";
import { type Focus, type Fold, foldFor, gridColumns } from "./fold";

describe("foldFor", () => {
  it.each([
    [1440, "wide"],
    [1920, "wide"],
    [1439, "folded"],
    [1200, "folded"],
    [1000, "folded"],
    [999, "narrow"],
    [900, "narrow"],
  ] as [number, Fold][])("%i is %s", (width, fold) => {
    expect(foldFor(width)).toBe(fold);
  });
});

describe("gridColumns", () => {
  const table: Record<Fold, Record<Focus, string>> = {
    wide: { split: "52px 1fr 340px", viewer: "52px 1fr", board: "52px 1fr" },
    folded: { split: "52px 1fr 52px", viewer: "52px 1fr", board: "52px 1fr" },
    narrow: { split: "52px 1fr", viewer: "52px 1fr", board: "52px 1fr" },
  };
  for (const fold of Object.keys(table) as Fold[]) {
    for (const focus of Object.keys(table[fold]) as Focus[]) {
      it(`${fold} ${focus}`, () => {
        expect(gridColumns(fold, focus)).toBe(table[fold][focus]);
      });
    }
  }
});
