import { useQuery } from "@tanstack/react-query";
import type { Status } from "@/generated/Status";
import type { TicketList } from "@/generated/TicketList";
import { call } from "@/lib/call";
import { unwrap } from "./envelope";

/** Keys are `[root, call, args]`; these calls take no args. S-0019 invalidates by
 *  these keys. */
export const statusKey = (root: string | null) => [root, "status"] as const;
export const ticketsKey = (root: string | null) => [root, "tickets_list"] as const;

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
