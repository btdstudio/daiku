import { describe, expect, it, vi } from "vitest";
import { makeQueryClient } from "./queryClient";

describe("makeQueryClient", () => {
  it("does not retry, treats data as stale and refetches on window focus", () => {
    const queries = makeQueryClient().getDefaultOptions().queries;
    expect(queries).toMatchObject({
      retry: false,
      refetchOnWindowFocus: true,
      staleTime: 0,
    });
  });

  it("tries a failed read once", async () => {
    const client = makeQueryClient();
    const queryFn = vi.fn(async () => {
      throw new Error("boom");
    });
    await expect(client.fetchQuery({ queryKey: ["k"], queryFn })).rejects.toThrow("boom");
    expect(queryFn).toHaveBeenCalledTimes(1);
  });

  it("keeps the cached data when a refetch fails", async () => {
    const client = makeQueryClient();
    await client.fetchQuery({ queryKey: ["k"], queryFn: async () => ({ n: 1 }) });

    await expect(
      client.fetchQuery({
        queryKey: ["k"],
        queryFn: async () => {
          throw new Error("half a file");
        },
      }),
    ).rejects.toThrow("half a file");

    expect(client.getQueryData(["k"])).toEqual({ n: 1 });
    expect(client.getQueryState(["k"])?.status).toBe("error");
  });
});
