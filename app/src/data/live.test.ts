import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";
import {
  adrsKey,
  artifactsKey,
  ideasKey,
  nextKey,
  readArtifactKey,
  readTextKey,
  statusKey,
  ticketsKey,
} from "./hooks";
import {
  applyWorkspaceChanged,
  changeMatches,
  invalidateAll,
  invalidateChanged,
  ticketPath,
} from "./live";

/** One representative query per command, seeded into every test's client. */
const SEEDED = {
  status: statusKey("/w"),
  tickets: ticketsKey("/w"),
  next: nextKey("/w"),
  ideas: ideasKey("/w"),
  adrs: adrsKey("/w"),
  artifactsA: artifactsKey("/w", "4-x"),
  artifactsIntake: artifactsKey("/w", "3-auth-flow"),
  artifactsBuild: artifactsKey("/w", "S-0019"),
  artifactsB: artifactsKey("/w", "5-y"),
  readArtifactA: readArtifactKey("/w", "4-x", "functional-spec.md"),
  readArtifactIntake: readArtifactKey("/w", "3-auth-flow", "functional-spec.md"),
  readArtifactBuild: readArtifactKey("/w", "S-0019", "requirements.md"),
  readTextPlan: readTextKey("/w", "plan/specs/4-x/tickets/S-1.md"),
  readTextInbox: readTextKey("/w", "build/tickets/N-1.md"),
};

/** Calls the query function recorded, so a refetch is visible by key. */
interface Client {
  client: QueryClient;
  fetched: string[];
}

function makeClient(): Client {
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
  return { client, fetched };
}

/** Cache every seeded key with data, so invalidations find them all. */
function seed(client: QueryClient): void {
  for (const key of Object.values(SEEDED)) client.setQueryData(key, "data");
}

/** The seeded keys that were invalidated (an inactive match is only marked stale). */
function hit(client: QueryClient): string[] {
  return Object.entries(SEEDED)
    .filter(([, key]) => client.getQueryState(key)?.isInvalidated === true)
    .map(([name]) => name);
}

/** Give a query an observer, so it counts as active and a refetch is triggered. */
function active(client: QueryClient, key: readonly unknown[]): () => void {
  const observer = new QueryObserver(client, { queryKey: key });
  return observer.subscribe(() => {});
}

describe("invalidateChanged", () => {
  it("State refetches status, next and every artifacts query, for its root only", async () => {
    const { client } = makeClient();
    seed(client);
    client.setQueryData(statusKey("/elsewhere"), "data");

    await invalidateChanged(client, "/w", { kind: "state" });

    expect(hit(client).sort()).toEqual(["artifactsA", "artifactsB", "artifactsBuild", "artifactsIntake", "next", "status"]);
    expect(client.getQueryState(statusKey("/elsewhere"))?.isInvalidated).toBe(false);
  });

  it("Ticket refreshes the list, next and the plan ticket's exact read_text", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "ticket", id: "S-1", change: "4-x" });

    expect(hit(client).sort()).toEqual([
      "artifactsA",
      "next",
      "readArtifactA",
      "readTextPlan",
      "tickets",
    ]);
  });

  it("Ticket without a change refreshes the inbox ticket's read_text", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "ticket", id: "N-1", change: null });

    expect(hit(client).sort()).toEqual(["next", "readTextInbox", "tickets"]);
  });

  it("Ticket with a missing change field is an inbox ticket too", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "ticket", id: "N-1" });

    expect(hit(client).sort()).toEqual(["next", "readTextInbox", "tickets"]);
  });

  it("PlanArtifact refreshes that change's artifacts and read_artifact", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "planArtifact", change: "4-x" });

    expect(hit(client).sort()).toEqual(["artifactsA", "readArtifactA"]);
  });

  it("PlanArtifact with a numeric intake change matches the plan name beginning n-", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "planArtifact", change: "3" });

    expect(hit(client).sort()).toEqual(["artifactsIntake", "readArtifactIntake"]);
  });

  it("BuildArtifact refreshes that build change's artifacts and read_artifact", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "buildArtifact", repo: "daiku", change: "S-0019" });

    expect(hit(client).sort()).toEqual(["artifactsBuild", "readArtifactBuild"]);
  });

  it("Idea refreshes idea_list only", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "idea", id: 6 });

    expect(hit(client)).toEqual(["ideas"]);
  });

  it("Adr refreshes adr_list only", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "adr" });

    expect(hit(client)).toEqual(["adrs"]);
  });

  it("Repos and Settings refresh status", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "repos" });
    expect(hit(client)).toEqual(["status"]);

    await invalidateChanged(client, "/w", { kind: "settings" });
    expect(hit(client)).toEqual(["status"]);
  });

  it("Reference and Scaffold refresh no core query", async () => {
    const { client } = makeClient();
    seed(client);

    await invalidateChanged(client, "/w", { kind: "reference" });
    await invalidateChanged(client, "/w", { kind: "scaffold" });

    expect(hit(client)).toEqual([]);
  });

  it("refetches an active match and only marks an inactive one stale", async () => {
    const { client, fetched } = makeClient();
    seed(client);
    const off = active(client, SEEDED.status);
    try {
      await invalidateChanged(client, "/w", { kind: "state" });
    } finally {
      off();
    }

    expect(fetched).toContain(JSON.stringify(SEEDED.status));
    expect(fetched).not.toContain(JSON.stringify(SEEDED.next));
    expect(client.getQueryState(SEEDED.status)?.isInvalidated).toBe(false);
    expect(client.getQueryState(SEEDED.next)?.isInvalidated).toBe(true);
    expect(client.getQueryState(SEEDED.tickets)?.isInvalidated).toBe(false);
  });
});

describe("invalidateAll", () => {
  it("refetches every cached core query for the root, inactive ones included", async () => {
    const { client, fetched } = makeClient();
    seed(client);
    client.setQueryData(["/w", "measure_root"], "not a core read");
    client.setQueryData(statusKey("/elsewhere"), "data");

    await invalidateAll(client, "/w");

    for (const key of Object.values(SEEDED)) {
      expect(fetched).toContain(JSON.stringify(key));
    }
    expect(fetched).not.toContain(JSON.stringify(["/w", "measure_root"]));
    expect(fetched).not.toContain(JSON.stringify(statusKey("/elsewhere")));
    expect(client.getQueryState(["/w", "measure_root"])?.isInvalidated).toBe(false);
    expect(client.getQueryState(statusKey("/elsewhere"))?.isInvalidated).toBe(false);
  });
});

describe("applyWorkspaceChanged", () => {
  it("applies each classified change of a normal payload", async () => {
    const { client } = makeClient();
    seed(client);

    await applyWorkspaceChanged(client, {
      root: "/w",
      changed: [{ kind: "state" }, { kind: "adr" }],
      reloadAll: false,
    });

    expect(hit(client).sort()).toEqual([
      "adrs",
      "artifactsA",
      "artifactsB",
      "artifactsBuild",
      "artifactsIntake",
      "next",
      "status",
    ]);
  });

  it("does nothing for a normal payload with no changes", async () => {
    const { client, fetched } = makeClient();
    seed(client);

    await applyWorkspaceChanged(client, { root: "/w", changed: [], reloadAll: false });

    expect(hit(client)).toEqual([]);
    expect(fetched).toEqual([]);
  });

  it("reloads the whole root from a reloadAll payload, whatever it carries", async () => {
    const { client, fetched } = makeClient();
    seed(client);
    client.setQueryData(ideasKey("/elsewhere"), "data");

    await applyWorkspaceChanged(client, {
      root: "/w",
      changed: [{ kind: "reference" }],
      reloadAll: true,
    });

    expect(fetched).toContain(JSON.stringify(SEEDED.status));
    expect(fetched).toContain(JSON.stringify(SEEDED.readTextInbox));
    expect(client.getQueryState(ideasKey("/elsewhere"))?.isInvalidated).toBe(false);
  });
});

describe("changeMatches", () => {
  it("matches an exact change name", () => {
    expect(changeMatches("4-x", "4-x")).toBe(true);
    expect(changeMatches("S-0019", "S-0019")).toBe(true);
    expect(changeMatches("4-x", "5-y")).toBe(false);
  });

  it("matches the numeric intake form against the plan change beginning n-", () => {
    expect(changeMatches("3-auth-flow", "3")).toBe(true);
    expect(changeMatches("3", "3")).toBe(true);
    expect(changeMatches("4-x", "3")).toBe(false);
    expect(changeMatches("33-x", "3")).toBe(false);
  });

  it("matches nothing for an absent change", () => {
    expect(changeMatches(null, "3")).toBe(false);
    expect(changeMatches(null, "4-x")).toBe(false);
  });
});

describe("ticketPath", () => {
  it("names the plan ticket or the inbox ticket", () => {
    expect(ticketPath("S-1", "4-x")).toBe("plan/specs/4-x/tickets/S-1.md");
    expect(ticketPath("N-1", null)).toBe("build/tickets/N-1.md");
  });
});
