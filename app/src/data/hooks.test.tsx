import { QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/call", () => ({ call: vi.fn() }));

import { call } from "@/lib/call";
import { status } from "@/board/fixtures";
import { failureText, FailureError } from "@/data/envelope";
import type { Status } from "@/generated/Status";
import type { Envelope } from "@/lib/envelope";
import { readTextKey, useReadText, useStatus } from "./hooks";
import { makeQueryClient } from "./queryClient";

const success = <T,>(data: T, warnings: string[] = []): Envelope<T> => ({
  schema: 1,
  data,
  warnings,
});

const failure = (kind: string, message: string): Envelope<never> => ({
  schema: 1,
  error: { kind, message },
});

function setup() {
  const client = makeQueryClient();
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, Wrapper };
}

describe("useCoreQuery", () => {
  beforeEach(() => vi.mocked(call).mockReset());

  it("gives the consumer the envelope's data and warnings", async () => {
    const s: Status = status([]);
    vi.mocked(call).mockResolvedValue(success(s, ["one", "two"]));
    const { Wrapper } = setup();
    const { result } = renderHook(() => useStatus("/w"), { wrapper: Wrapper });

    await waitFor(() => expect(result.current.data).toBe(s));
    expect(result.current.warnings).toEqual(["one", "two"]);
    expect(result.current.error).toBeNull();
    expect(call).toHaveBeenCalledWith("status", { root: "/w" });
  });

  it("replaces the warnings on a successful refetch", async () => {
    const s: Status = status([]);
    vi.mocked(call).mockResolvedValue(success(s, ["one"]));
    const { Wrapper, client } = setup();
    const { result } = renderHook(() => useStatus("/w"), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.warnings).toEqual(["one"]));

    vi.mocked(call).mockResolvedValue(success(s, ["two"]));
    await client.refetchQueries();
    await waitFor(() => expect(result.current.warnings).toEqual(["two"]));
    expect(result.current.data).toBe(s);
  });

  it("rejects a Failure envelope with FailureError", async () => {
    vi.mocked(call).mockResolvedValue(failure("Syntax", "unexpected end of file"));
    const { Wrapper } = setup();
    const { result } = renderHook(() => useStatus("/w"), { wrapper: Wrapper });

    await waitFor(() => expect(result.current.error).toBeInstanceOf(FailureError));
    expect(result.current.data).toBeUndefined();
    expect(result.current.warnings).toEqual([]);
    expect(failureText(result.current.error)).toBe("Syntax: unexpected end of file");
  });

  it("keeps the last good data and warnings while a refetch has failed", async () => {
    const s: Status = status([]);
    vi.mocked(call).mockResolvedValue(success(s, ["kept"]));
    const { Wrapper, client } = setup();
    const { result } = renderHook(() => useStatus("/w"), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.warnings).toEqual(["kept"]));

    vi.mocked(call).mockResolvedValue(failure("Io", "disk went away"));
    await client.refetchQueries();
    await waitFor(() => expect(result.current.error).toBeInstanceOf(FailureError));
    expect(result.current.data).toBe(s);
    expect(result.current.warnings).toEqual(["kept"]);
    expect(failureText(result.current.error)).toBe("Io: disk went away");
  });

  it("clears the error and shows current data on the next success", async () => {
    const first: Status = status([], ["S-1"]);
    const second: Status = status([], ["S-1", "S-2"]);
    vi.mocked(call).mockResolvedValue(success(first, ["old"]));
    const { Wrapper, client } = setup();
    const { result } = renderHook(() => useStatus("/w"), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.warnings).toEqual(["old"]));

    vi.mocked(call).mockResolvedValue(failure("Syntax", "truncated"));
    await client.refetchQueries();
    await waitFor(() => expect(result.current.error).toBeInstanceOf(FailureError));

    vi.mocked(call).mockResolvedValue(success(second, ["new"]));
    await client.refetchQueries();
    await waitFor(() => expect(result.current.error).toBeNull());
    expect(result.current.data).toEqual(second);
    expect(result.current.warnings).toEqual(["new"]);
  });
});

describe("useReadText", () => {
  beforeEach(() => vi.mocked(call).mockReset());

  it("keys and calls read_text with the root and relative path", async () => {
    expect(readTextKey("/w", "build/tickets/N-0001.md")).toEqual([
      "/w",
      "read_text",
      "build/tickets/N-0001.md",
    ]);
    vi.mocked(call).mockResolvedValue(success("# Ticket", ["no front matter"]));
    const { Wrapper, client } = setup();
    const { result } = renderHook(() => useReadText("/w", "build/tickets/N-0001.md"), {
      wrapper: Wrapper,
    });

    await waitFor(() => expect(result.current.data).toBe("# Ticket"));
    expect(result.current.warnings).toEqual(["no front matter"]);
    expect(call).toHaveBeenCalledWith("read_text", {
      root: "/w",
      rel: "build/tickets/N-0001.md",
    });
    expect(client.getQueryState(readTextKey("/w", "build/tickets/N-0001.md"))?.status).toBe(
      "success",
    );
  });

  it("stays off while either argument is null", async () => {
    const { Wrapper, client } = setup();
    const { result } = renderHook(() => useReadText("/w", null), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.isPending).toBe(true));
    expect(call).not.toHaveBeenCalled();
    expect(result.current.data).toBeUndefined();
    expect(result.current.warnings).toEqual([]);
    expect(client.getQueryState(readTextKey("/w", null))?.fetchStatus).toBe("idle");
  });
});
