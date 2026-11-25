import { type Query, type QueryClient, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { Changed } from "@/generated/Changed";

/** The event one root's watcher emits (`app/src-tauri/src/watch.rs`). */
const EVENT = "workspace-changed";

/** What a changed path means for the views, as the watcher sends it: the root that
 *  changed, the classified changes, or `reloadAll` when events were lost. */
export interface WorkspaceChanged {
  root: string;
  changed: Changed[];
  reloadAll: boolean;
}

/** Every cached core read, by command; a `Changed` never names a lifecycle command. */
const CORE_READS = new Set([
  "status",
  "tickets_list",
  "next",
  "idea_list",
  "adr_list",
  "artifacts",
  "read_artifact",
  "read_ticket",
]);

/** The part of a query this map reads. */
interface Keyed {
  readonly queryKey: readonly unknown[];
}

/** Only a key whose first element is the payload's root is ever matched. */
const forRoot = (q: Keyed, root: string) => q.queryKey[0] === root;

/** A `[root, call, ...args]` key's command. */
function command(q: Keyed): string | null {
  const v = q.queryKey[1];
  return typeof v === "string" ? v : null;
}

/** A key argument, for the ones the key factories pass as strings. */
function arg(q: Keyed, i: number): string | null {
  const v = q.queryKey[i];
  return typeof v === "string" ? v : null;
}

/**
 * Whether a query's change argument names the change a `Changed` reports: the same
 * change name, or the numeric intake form (`plan/00_intake/<n>.md` classifies as `n`)
 * against the plan change beginning `n-`.
 */
export function changeMatches(change: string | null, changed: string): boolean {
  if (change === changed) return true;
  if (change === null || !/^\d+$/.test(changed)) return false;
  return change.startsWith(`${changed}-`);
}

/** Cancel the matched queries still on their first fetch. Invalidating would otherwise
 *  reuse that fetch, whose answer may predate the change. */
function cancelFirstFetches(
  client: QueryClient,
  predicate: (q: Query) => boolean,
): Promise<void> {
  return client.cancelQueries({
    predicate: (q) =>
      predicate(q) && q.state.data === undefined && q.state.fetchStatus === "fetching",
  });
}

/** Invalidate the root's keys `test` matches: active ones refetch, inactive ones go
 *  stale (TanStack's default `refetchType: "active"`). */
async function invalidate(
  client: QueryClient,
  root: string,
  test: (q: Keyed) => boolean,
): Promise<void> {
  const predicate = (q: Query) => forRoot(q, root) && test(q);
  await cancelFirstFetches(client, predicate);
  await client.invalidateQueries({ predicate });
}

/** Whether a query is one of `commands`. */
const reads =
  (...commands: string[]) =>
  (q: Keyed) => {
    const cmd = command(q);
    return cmd !== null && commands.includes(cmd);
  };

/** Whether a query reads the files of a change `matches` accepts. */
function changeFiles(q: Keyed, ...changes: string[]): boolean {
  return (
    reads("artifacts", "read_artifact")(q) && changes.some((c) => changeMatches(arg(q, 2), c))
  );
}

/**
 * The design's invalidation map for one `Changed` value and one root: only that root's
 * queries, only the commands the change affects (design D5).
 */
export function invalidateChanged(
  client: QueryClient,
  root: string,
  changed: Changed,
): Promise<void> {
  switch (changed.kind) {
    case "state":
      // `artifacts` carries the change and phase data as well as paths, and the ticket
      // list says which plans are closed.
      return invalidate(client, root, reads("status", "next", "tickets_list", "artifacts"));
    case "ticket": {
      // A build change is named by its ticket's id, and lists the ticket as its source.
      // Every ticket read goes: its key holds the path as shown, which may be escaped.
      const changes = [changed.id, ...(changed.change ? [changed.change] : [])];
      return invalidate(
        client,
        root,
        (q) =>
          reads("status", "tickets_list", "next", "read_ticket")(q) || changeFiles(q, ...changes),
      );
    }
    case "planArtifact":
      // A ticket folder moved or renamed whole is reported only as its plan's artifact.
      return invalidate(
        client,
        root,
        (q) =>
          reads("status", "tickets_list", "next", "read_ticket")(q) ||
          changeFiles(q, changed.change),
      );
    case "buildArtifact":
      return invalidate(client, root, (q) => changeFiles(q, changed.change));
    case "idea":
      return invalidate(client, root, reads("idea_list"));
    case "adr":
      return invalidate(client, root, reads("adr_list"));
    case "repos":
      return invalidate(client, root, reads("status", "next"));
    case "settings":
      return invalidate(client, root, reads("status", "adr_list"));
    case "reference":
    case "scaffold":
      // Kept in the typed event contract for later views; no core query reads them yet.
      return Promise.resolve();
  }
}

/** Every cached core query for the root, inactive ones included (`refetchType: "all"`). */
export async function invalidateAll(client: QueryClient, root: string): Promise<void> {
  const predicate = (q: Query) => {
    const cmd = command(q);
    return forRoot(q, root) && cmd !== null && CORE_READS.has(cmd);
  };
  await cancelFirstFetches(client, predicate);
  await client.invalidateQueries({ predicate, refetchType: "all" });
}

/** Apply one `workspace-changed` payload: `reloadAll` refreshes every core query for the
 *  root; otherwise only the queries its classified changes name. */
export async function applyWorkspaceChanged(
  client: QueryClient,
  payload: WorkspaceChanged,
): Promise<void> {
  if (payload.reloadAll) {
    await invalidateAll(client, payload.root);
    return;
  }
  for (const changed of payload.changed) {
    await invalidateChanged(client, payload.root, changed);
  }
}

/**
 * The app's one live-updates hook (design D5): registers a single `workspace-changed`
 * listener and, once it's in place, sends the complete open-root set to `sync_watchers`,
 * so a new watcher's first event isn't missed. Watcher lifetime is the backend
 * registry's; only the listener is cleaned up here.
 */
export function useLiveUpdates(openRoots: readonly string[]): void {
  const client = useQueryClient();
  const rootsKey = JSON.stringify(openRoots);
  const listening = useRef<Promise<unknown>>(Promise.resolve());

  useEffect(() => {
    const listener = listen<WorkspaceChanged>(EVENT, (e) => {
      void applyWorkspaceChanged(client, e.payload);
    }).catch((err) => {
      console.error("workspace-changed listener failed", err);
      return () => {};
    });
    listening.current = listener;
    return () => {
      void listener.then((unlisten) => unlisten());
    };
  }, [client]);

  useEffect(() => {
    const roots = JSON.parse(rootsKey) as string[];
    // Only the latest set is sent if it changes while the listener is still pending.
    let superseded = false;
    void listening.current.then(() => {
      if (superseded) return;
      invoke("sync_watchers", { roots }).catch((err) =>
        console.error("sync_watchers failed", err),
      );
    });
    return () => {
      superseded = true;
    };
  }, [client, rootsKey]);
}
