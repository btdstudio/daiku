import { QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { fail, idea } from "@/board/fixtures";
import { makeQueryClient } from "@/data/queryClient";
import { IdeaPanel } from "./IdeaPanel";

/** A success envelope carrying core's warnings. */
const warns = (data: unknown, warnings: string[]) => ({ schema: 1, data, warnings });

function show() {
  const client = makeQueryClient();
  render(
    <QueryClientProvider client={client}>
      <IdeaPanel root="/w" id={3} focused={false} onFill={() => {}} />
    </QueryClientProvider>,
  );
  return { client };
}

describe("IdeaPanel", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());
  afterEach(cleanup);

  it("shows the idea's warnings beside its fields and body", async () => {
    vi.mocked(invoke).mockResolvedValue(warns([idea(3, { body: "# Body" })], ["ideas warning"]));
    show();
    await screen.findByText("Body");
    expect(screen.getByText("ideas warning")).toBeTruthy();
    expect(screen.getByText("Idea 3")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows a failure as Kind: message and keeps the last idea after a failed refetch", async () => {
    vi.mocked(invoke).mockResolvedValue(warns([idea(3, { body: "# Body" })], ["kept warning"]));
    const { client } = show();
    await screen.findByText("Body");
    vi.mocked(invoke).mockResolvedValue(fail("Io", "disk went away"));
    await client.refetchQueries();
    expect((await screen.findByRole("alert")).textContent).toBe("Io: disk went away");
    expect(screen.getByText("Body")).toBeTruthy();
    expect(screen.getByText("kept warning")).toBeTruthy();
  });
});
