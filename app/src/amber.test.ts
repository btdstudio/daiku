import { describe, expect, it } from "vitest";

// Amber (the `attention` colours) is spelled in exactly these files. Every other surface
// uses NEXT_MARK, DRAFT_INK or StatusText, so it never names the colour. Extending this
// list is only for spec R20's four uses: the nudge, the next mark, the rail's count and
// the draft labels. Anything else needs a change to R20 first.
const ALLOWED = ["frame/ListRail.tsx", "frame/StatusBar.tsx", "frame/marks.ts"];

// Every source under src/ as raw text, except generated types, the theme, and tests.
const sources = import.meta.glob(
  ["./**/*.{ts,tsx}", "!./generated/**", "!./theme/**", "!./**/*.test.*"],
  { query: "?raw", import: "default", eager: true },
) as Record<string, string>;

describe("the amber audit", () => {
  it("names amber only in the allowlisted files", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(40);
    const users = Object.entries(sources)
      .filter(([, text]) => /attention|amber/i.test(text))
      .map(([file]) => file.replace(/^\.\//, ""))
      .sort();
    expect(users).toEqual(ALLOWED);
  });
});
