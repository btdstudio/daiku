import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
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
  "read_text",
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

/**
 * The workspace-relative path a `Changed::Ticket` names, as `TicketView.path` and
 * `read_text` use it: the plan change's ticket file, or the inbox one without a change.
 */
export function ticketPath(id: string, change: string | null): string {
  return change === null ? `build/tickets/${id}.md` : `plan/specs/${change}/tickets/${id}.md`;
}

/** Invalidate the root's keys `test` matches: active ones refetch, inactive ones go
 *  stale (TanStack's default `refetchType: "active"`). */
function invalidate(
  client: QueryClient,
  root: string,
  test: (q: Keyed) => boolean,
): Promise<void> {
  return client.invalidateQueries({ predicate: (q) => forRoot(q, root) && test(q) });
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
      // `artifacts` carries the change and phase data as well as paths.
      return invalidate(client, root, (q) => {
        const cmd = command(q);
        return cmd === "status" || cmd === "next" || cmd === "artifacts";
      });
    case "ticket": {
      const change = changed.change ?? null;
      const rel = ticketPath(changed.id, change);
      return invalidate(client, root, (q) => {
        const cmd = command(q);
        if (cmd === "tickets_list" || cmd === "next") return true;
        if (cmd === "read_text") return arg(q, 2) === rel;
        if (change === null) return false;
        return (
          (cmd === "artifacts" || cmd === "read_artifact") && changeMatches(arg(q, 2), change)
        );
      });
    }
    case "planArtifact":
    case "buildArtifact":
      return invalidate(client, root, (q) => {
        const cmd = command(q);
        return (
          (cmd === "artifacts" || cmd === "read_artifact") && changeMatches(arg(q, 2), changed.change)
        );
      });
    case "idea":
      return invalidate(client, root, (q) => command(q) === "idea_list");
    case "adr":
      return invalidate(client, root, (q) => command(q) === "adr_list");
    case "repos":
    case "settings":
      return invalidate(client, root, (q) => command(q) === "status");
    case "reference":
    case "scaffold":
      // Kept in the typed event contract for later views; no core query reads them yet.
      return Promise.resolve();
  }
}

/** Every cached core query for the root, inactive ones included (`refetchType: "all"`). */
export function invalidateAll(client: QueryClient, root: string): Promise<void> {
  return client.invalidateQueries({
    predicate: (q) => {
      const cmd = command(q);
      return forRoot(q, root) && cmd !== null && CORE_READS.has(cmd);
    },
    refetchType: "all",
  });
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
 * listener and sends the complete open-root set to `sync_watchers`. Watcher lifetime
 * is the backend registry's; only the listener is cleaned up here.
 */
export function useLiveUpdates(openRoots: readonly string[]): void {
  const client = useQueryClient();
  const rootsKey = JSON.stringify(openRoots);

  useEffect(() => {
    const listener = listen<WorkspaceChanged>(EVENT, (e) => {
      void applyWorkspaceChanged(client, e.payload);
    }).catch((err) => {
      console.error("workspace-changed listener failed", err);
      return () => {};
    });
    return () => {
      void listener.then((unlisten) => unlisten());
    };
  }, [client]);

  useEffect(() => {
    const roots = JSON.parse(rootsKey) as string[];
    invoke("sync_watchers", { roots }).catch((err) =>
      console.error("sync_watchers failed", err),
    );
  }, [rootsKey]);
}
