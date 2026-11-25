import { QueryClient } from "@tanstack/react-query";

/** The app's query client. A failed refetch keeps the last data (spec R25), and a
 *  window focus refetches the live reads (R24); the file watcher also invalidates them
 *  (S-0019). */
export function makeQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, refetchOnWindowFocus: true, staleTime: 0 },
    },
  });
}

export const queryClient = makeQueryClient();
