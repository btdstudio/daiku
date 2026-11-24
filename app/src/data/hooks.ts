import { useQuery } from "@tanstack/react-query";
import type { ArtifactList } from "@/generated/ArtifactList";
import type { Idea } from "@/generated/Idea";
import type { NextList } from "@/generated/NextList";
import type { Record as Adr } from "@/generated/AdrRecord";
import type { Status } from "@/generated/Status";
import type { TicketList } from "@/generated/TicketList";
import { call } from "@/lib/call";
import { unwrap } from "./envelope";

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

export function useStatus(root: string | null) {
  return useQuery({
    queryKey: statusKey(root),
    queryFn: () => call<Status>("status", { root }).then(unwrap),
    enabled: root !== null,
  });
}

export function useTickets(root: string | null) {
  return useQuery({
    queryKey: ticketsKey(root),
    queryFn: () => call<TicketList>("tickets_list", { root }).then(unwrap),
    enabled: root !== null,
  });
}

/** What is waiting on the person (`items[0]` is the nudge). Refetches on window focus
 *  (the default); a failed refetch keeps the last data. S-0019 invalidates `nextKey`. */
export function useNext(root: string | null) {
  return useQuery({
    queryKey: nextKey(root),
    queryFn: () => call<NextList>("next", { root }).then(unwrap),
    enabled: root !== null,
  });
}

export function useIdeas(root: string | null) {
  return useQuery({
    queryKey: ideasKey(root),
    queryFn: () => call<Idea[]>("idea_list", { root }).then(unwrap),
    enabled: root !== null,
  });
}

export function useAdrs(root: string | null) {
  return useQuery({
    queryKey: adrsKey(root),
    queryFn: () => call<Adr[]>("adr_list", { root }).then(unwrap),
    enabled: root !== null,
  });
}

/** A change's files by phase; off until `change` is set. */
export function useArtifacts(root: string | null, change: string | null) {
  return useQuery({
    queryKey: artifactsKey(root, change),
    queryFn: () => call<ArtifactList>("artifacts", { root, change }).then(unwrap),
    enabled: root !== null && change !== null,
  });
}

/** One listed file's text, by its path as `artifacts` shows it; off until both are set. */
export function useReadArtifact(root: string | null, change: string | null, path: string | null) {
  return useQuery({
    queryKey: readArtifactKey(root, change, path),
    queryFn: () => call<string>("read_artifact", { root, change, path }).then(unwrap),
    enabled: root !== null && change !== null && path !== null,
  });
}
