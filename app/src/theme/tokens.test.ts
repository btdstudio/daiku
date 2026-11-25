// eslint-disable-next-line @typescript-eslint/ban-ts-comment -- test compiles under Vitest; app tsconfig has no node types.
// @ts-nocheck
import fs from "node:fs";
import path from "node:path";
import { compile } from "tailwindcss";
import { describe, expect, it } from "vitest";

async function buildCss(candidates: string[]): Promise<string> {
  const base = process.cwd();
  const indexCss = fs.readFileSync(path.join(base, "src/index.css"), "utf8");
  const compiler = await compile(indexCss, {
    base: path.join(base, "src"),
    loadStylesheet: async (id: string, from?: string) => {
      let p: string;
      if (id === "tw-animate-css") p = path.join(base, "node_modules/tw-animate-css/dist/tw-animate.css");
      else if (id === "shadcn/tailwind.css") p = path.join(base, "node_modules/shadcn/dist/tailwind.css");
      else if (id.startsWith(".")) p = path.resolve(from ?? path.join(base, "src"), id);
      else if (id === "tailwindcss") p = path.join(base, "node_modules/tailwindcss/index.css");
      else p = path.resolve(base, "node_modules", id);
      const content = fs.readFileSync(p, "utf8");
      return { path: p, base: path.dirname(p), content };
    },
  });
  return compiler.build(candidates);
}

describe("design tokens", () => {
  it("generates text-id and rounded-pill CSS (AC5)", async () => {
    const out = await buildCss(["text-id", "rounded-pill"]);
    expect(out).toContain(".text-id");
    expect(out).toContain("font-family: var(--font-mono-family)");
    expect(out).toContain("font-weight: 500");
    expect(out).toContain(".rounded-pill");
  });

  it("removes radius-window mapping (Q6)", async () => {
    const indexCss = fs.readFileSync(path.join(process.cwd(), "src/index.css"), "utf8");
    const tokensCss = fs.readFileSync(path.join(process.cwd(), "src/theme/tokens.css"), "utf8");
    expect(indexCss).not.toContain("--radius-xl");
    expect(indexCss).not.toContain("--radius-window");
    expect(tokensCss).not.toContain("--radius-window");
  });
});
