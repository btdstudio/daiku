import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { PLAN_PHASES, fail, ok, plan } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import type { ArtifactList } from "@/generated/ArtifactList";
import { PlanPanel } from "./PlanPanel";

const list = (over: Parameters<typeof plan>[1] = {}): ArtifactList => ({
  change: plan("4-plan", over),
  phases: PLAN_PHASES,
  artifacts: [
    { phase: "intake", path: "plan/00_intake/4.md" },
    { phase: "spec", path: "plan/specs/4-plan/a.md" },
    { phase: "spec", path: "plan/specs/4-plan/b.md" },
  ],
});

const TEXTS: Record<string, string> = {
  "plan/00_intake/4.md": "---\nid: 4\n---\n\n# Intake body",
  "plan/specs/4-plan/a.md": "# First spec",
  "plan/specs/4-plan/b.md": "# Second spec",
};

function serve(l: ArtifactList) {
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === "artifacts") return ok(l);
    const path = (args as { path?: string } | undefined)?.path ?? "";
    return path in TEXTS ? ok(TEXTS[path]) : fail("NotAnArtifact", "nope");
  });
}

function show(name = "4-plan", root = "/w", initialPhase?: string) {
  const client = makeQueryClient();
  const ui = (n: string, r: string, p?: string) => (
    <QueryClientProvider client={client}>
      <PlanPanel root={r} name={n} initialPhase={p} focused={false} onFill={() => {}} />
    </QueryClientProvider>
  );
  const view = render(ui(name, root, initialPhase));
  return {
    ...view,
    client,
    to: (n: string, r = root) => view.rerender(ui(n, r)),
    phase: (p?: string) => view.rerender(ui(name, root, p)),
  };
}

const chip = (name: string) => screen.getByRole("button", { name: new RegExp(`^${name}`) });

describe("PlanPanel", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows all phases, research skipped with its reason, the current phase's first file", async () => {
    serve(
      list({
        phase: "spec",
        gates: { intake: "approved", research: "skipped", spec: "draft" },
        reasons: { research: "no vendors" },
      }),
    );
    show();
    await screen.findByText("First spec");
    const chips = screen.getByRole("group", { name: "Phases" }).querySelectorAll("button");
    expect([...chips].map((b) => b.children[1].textContent)).toEqual(PLAN_PHASES);
    expect(chip("research").textContent).toContain("skipped");
    expect(chip("research").getAttribute("title")).toBe("no vendors");
    expect(chip("spec").getAttribute("aria-current")).toBe("step");
    expect(chip("explore").textContent).toContain("not started");
    expect(invoke).toHaveBeenCalledWith("read_artifact", {
      root: "/w",
      change: "4-plan",
      path: "plan/specs/4-plan/a.md",
    });
    // The skipped phase shows its reason inline once chosen.
    fireEvent.click(chip("research"));
    expect(screen.getByText("Skipped: no vendors")).toBeTruthy();
    expect(await screen.findByText("No files in this phase")).toBeTruthy();
    expect(chip("research").getAttribute("aria-pressed")).toBe("true");
  });

  it("starts at the first phase when change.phase is empty, with front matter above the body", async () => {
    serve(list({ phase: "" }));
    const { container } = show();
    await screen.findByText("Intake body");
    expect(chip("intake").getAttribute("aria-current")).toBe("step");
    expect(container.querySelector("pre[data-front-matter]")?.textContent).toBe("id: 4");
  });

  it("lists a phase's files in order and shows another when chosen", async () => {
    serve(list({ phase: "spec" }));
    show();
    await screen.findByText("First spec");
    const files = screen.getByRole("list", { name: "Files" }).querySelectorAll("button");
    expect([...files].map((b) => b.textContent)).toEqual([
      "plan/specs/4-plan/a.md",
      "plan/specs/4-plan/b.md",
    ]);
    fireEvent.click(files[1]);
    expect(await screen.findByText("Second spec")).toBeTruthy();
    expect(screen.queryByText("First spec")).toBeNull();
  });

  it("resets the choice when the plan changes", async () => {
    serve(list({ phase: "spec" }));
    const { to } = show();
    await screen.findByText("First spec");
    fireEvent.click(chip("intake"));
    await screen.findByText("Intake body");
    to("5-other");
    await waitFor(() => expect(screen.getByText("First spec")).toBeTruthy());
    expect(chip("spec").getAttribute("aria-pressed")).toBe("true");
  });

  it("starts on initialPhase and shows its first file", async () => {
    serve(list({ phase: "spec" }));
    show("4-plan", "/w", "intake");
    await screen.findByText("Intake body");
    expect(chip("intake").getAttribute("aria-pressed")).toBe("true");
    expect(chip("spec").getAttribute("aria-current")).toBe("step");
  });

  it("falls back to the current phase for an unknown initialPhase", async () => {
    serve(list({ phase: "spec" }));
    show("4-plan", "/w", "nonesuch");
    await screen.findByText("First spec");
    expect(chip("spec").getAttribute("aria-pressed")).toBe("true");
  });

  it("resets the pick when initialPhase changes", async () => {
    serve(list({ phase: "spec" }));
    const { phase } = show("4-plan", "/w", "intake");
    await screen.findByText("Intake body");
    fireEvent.click(chip("spec"));
    await screen.findByText("First spec");
    phase("spec");
    await screen.findByText("First spec");
    phase("intake");
    await screen.findByText("Intake body");
    expect(chip("intake").getAttribute("aria-pressed")).toBe("true");
  });

  it("shows a read failure inline", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) =>
      cmd === "artifacts" ? ok(list({ phase: "intake" })) : fail("TooLarge", "file too large to show (1.0 MiB)"),
    );
    show();
    expect((await screen.findByRole("alert")).textContent).toBe("File too large to show (1.0 MiB)");
  });

  it("keeps the last file with an error after a failed refetch", async () => {
    serve(list({ phase: "spec" }));
    const { client } = show();
    await screen.findByText("First spec");
    vi.mocked(invoke).mockImplementation(async (cmd: string) =>
      cmd === "artifacts" ? ok(list({ phase: "spec" })) : fail("Io", "disk went away"),
    );
    await client.refetchQueries();
    expect((await screen.findByRole("alert")).textContent).toContain("disk went away");
    expect(screen.getByText("First spec")).toBeTruthy();
  });
});
