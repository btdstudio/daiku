import { type QueryKey, type UseQueryResult, useQuery } from "@tanstack/react-query";
import type { ArtifactList } from "@/generated/ArtifactList";
import type { Idea } from "@/generated/Idea";
import type { NextList } from "@/generated/NextList";
import type { Record as Adr } from "@/generated/AdrRecord";
import type { Status } from "@/generated/Status";
import type { TicketList } from "@/generated/TicketList";
import { call } from "@/lib/call";
import type { Success } from "@/lib/envelope";
import { envelope, FailureError } from "./envelope";

/** A core read's query result: the envelope's data and its warnings, apart from the rest
 *  of the query. A failed refetch keeps the last successful envelope's pair. */
export type CoreQueryResult<T> = Omit<UseQueryResult<Success<T>, FailureError>, "data"> & {
  data: T | undefined;
  warnings: string[];
};

/** Shared, so a result without warnings doesn't allocate one each render. */
const NO_WARNINGS: string[] = [];

/** One core read: the query holds the whole `Success<T>` envelope, and the consumer sees
 *  its `data` and `warnings`. A `Failure` rejects with `FailureError`. `S-0019` invalidates
 *  by these keys. */
export function useCoreQuery<T>(
  queryKey: QueryKey,
  queryFn: () => Promise<Success<T>>,
  enabled: boolean,
): CoreQueryResult<T> {
  const q = useQuery<Success<T>, FailureError>({ queryKey, queryFn, enabled });
  return {
    ...q,
    data: q.data?.data,
    warnings: q.data?.warnings ?? NO_WARNINGS,
  };
}

/** Keys are `[root, call, args]`; these calls take no args. S-0019 invalidates by
 *  these keys. */
export const statusKey = (root: string | null) => [root, "status"] as const;
export const ticketsKey = (root: string | null) => [root, "tickets_list"] as const;

export const nextKey = (root: string | null) => [root, "next"] as const;

export const ideasKey = (root: string | null) => [root, "idea_list"] as const;
export const adrsKey = (root: string | null) => [root, "adr_list"] as const;
export const artifactsKey = (root: string | null, change: string | null) =>
  [root, "artifacts", change] as const;
export const readArtifactKey = (root: string | null, change: string | null, path: string | null) =>
  [root, "read_artifact", change, path] as const;
export const readTextKey = (root: string | null, rel: string | null) =>
  [root, "read_text", rel] as const;

export function useStatus(root: string | null) {
  return useCoreQuery(
    statusKey(root),
    () => call<Status>("status", { root }).then(envelope),
    root !== null,
  );
}

export function useTickets(root: string | null) {
  return useCoreQuery(
    ticketsKey(root),
    () => call<TicketList>("tickets_list", { root }).then(envelope),
    root !== null,
  );
}

/** What is waiting on the person (`items[0]` is the nudge). Refetches on window focus
 *  (the default); a failed refetch keeps the last data. S-0019 invalidates `nextKey`. */
export function useNext(root: string | null) {
  return useCoreQuery(
    nextKey(root),
    () => call<NextList>("next", { root }).then(envelope),
    root !== null,
  );
}

export function useIdeas(root: string | null) {
  return useCoreQuery(
    ideasKey(root),
    () => call<Idea[]>("idea_list", { root }).then(envelope),
    root !== null,
  );
}

export function useAdrs(root: string | null) {
  return useCoreQuery(
    adrsKey(root),
    () => call<Adr[]>("adr_list", { root }).then(envelope),
    root !== null,
  );
}

/** A change's files by phase; off until `change` is set. */
export function useArtifacts(root: string | null, change: string | null) {
  return useCoreQuery(
    artifactsKey(root, change),
    () => call<ArtifactList>("artifacts", { root, change }).then(envelope),
    root !== null && change !== null,
  );
}

/** One listed file's text, by its path as `artifacts` shows it; off until both are set. */
export function useReadArtifact(root: string | null, change: string | null, path: string | null) {
  return useCoreQuery(
    readArtifactKey(root, change, path),
    () => call<string>("read_artifact", { root, change, path }).then(envelope),
    root !== null && change !== null && path !== null,
  );
}

/** One open file's text, by its workspace-relative path; off until both are set. */
export function useReadText(root: string | null, rel: string | null): CoreQueryResult<string> {
  return useCoreQuery(
    readTextKey(root, rel),
    () => call<string>("read_text", { root, rel }).then(envelope),
    root !== null && rel !== null,
  );
}
