import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { Event } from "@tauri-apps/api/event";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { statusKey } from "./hooks";
import { useLiveUpdates, type WorkspaceChanged } from "./live";

const unlisten = vi.fn();
let deliver: ((payload: WorkspaceChanged) => void) | null = null;

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async () => undefined);
  vi.mocked(listen).mockReset();
  unlisten.mockReset();
  deliver = null;
  vi.mocked(listen).mockImplementation(async (_event, cb) => {
    const handler = cb as unknown as (e: Event<WorkspaceChanged>) => void;
    deliver = (payload) => handler({ event: "workspace-changed", id: 1, payload });
    return unlisten;
  });
});

afterEach(cleanup);

function setup(initialRoots: string[]) {
  const fetched: string[] = [];
  const client = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
        queryFn: async (ctx) => {
          fetched.push(JSON.stringify(ctx.queryKey));
          return "data";
        },
      },
    },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const view = renderHook(({ roots }: { roots: string[] }) => useLiveUpdates(roots), {
    initialProps: { roots: initialRoots },
    wrapper: Wrapper,
  });
  return { client, fetched, ...view };
}

/** The `sync_watchers` calls made so far, in order. */
const synced = () =>
  vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "sync_watchers").map(([, args]) => args);

describe("useLiveUpdates", () => {
  it("registers one listener and sends the restored open roots", async () => {
    const { unmount } = setup(["/a", "/b"]);

    await waitFor(() => expect(synced()).toEqual([{ roots: ["/a", "/b"] }]));
    expect(listen).toHaveBeenCalledTimes(1);
    expect(vi.mocked(listen).mock.calls[0][0]).toBe("workspace-changed");

    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
    expect(synced()).toEqual([{ roots: ["/a", "/b"] }]);
  });

  it("syncs only once the listener is in place, with the latest set", async () => {
    let ready: (off: () => void) => void = () => {};
    vi.mocked(listen).mockImplementation(
      () =>
        new Promise((resolve) => {
          ready = resolve;
        }),
    );
    const { rerender } = setup(["/a"]);
    rerender({ roots: ["/a", "/b"] });
    await act(async () => {});
    expect(synced()).toEqual([]);

    await act(async () => ready(unlisten));

    expect(synced()).toEqual([{ roots: ["/a", "/b"] }]);
  });

  it("keeps the one listener while the open set grows", async () => {
    const { rerender } = setup(["/a"]);
    await waitFor(() => expect(synced()).toEqual([{ roots: ["/a"] }]));

    rerender({ roots: ["/a", "/b"] });

    await waitFor(() =>
      expect(synced()).toEqual([{ roots: ["/a"] }, { roots: ["/a", "/b"] }]),
    );
    expect(listen).toHaveBeenCalledTimes(1);
    expect(unlisten).not.toHaveBeenCalled();
  });

  it("reconciles to the remaining roots when one closes", async () => {
    const { rerender } = setup(["/a", "/b"]);
    await waitFor(() => expect(synced()).toEqual([{ roots: ["/a", "/b"] }]));

    rerender({ roots: ["/b"] });

    await waitFor(() =>
      expect(synced()).toEqual([{ roots: ["/a", "/b"] }, { roots: ["/b"] }]),
    );
  });

  it("invalidates only the payload's root when an event arrives", async () => {
    const { client } = setup(["/a", "/b"]);
    await waitFor(() => expect(deliver).not.toBeNull());
    client.setQueryData(statusKey("/a"), "data");
    client.setQueryData(statusKey("/b"), "data");

    act(() => deliver!({ root: "/a", changed: [{ kind: "repos" }], reloadAll: false }));

    await waitFor(() => expect(client.getQueryState(statusKey("/a"))?.isInvalidated).toBe(true));
    expect(client.getQueryState(statusKey("/b"))?.isInvalidated).toBe(false);
  });

  it("refetches every core query for the root on a reloadAll event", async () => {
    const { client, fetched } = setup(["/a"]);
    await waitFor(() => expect(deliver).not.toBeNull());
    client.setQueryData(statusKey("/a"), "data");

    act(() => deliver!({ root: "/a", changed: [], reloadAll: true }));

    await waitFor(() =>
      expect(fetched).toContain(JSON.stringify(statusKey("/a"))),
    );
  });
});
