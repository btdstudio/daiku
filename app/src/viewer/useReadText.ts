import { useEffect, useState } from "react";
import type { Failure } from "@/generated/Failure";
import { call } from "@/lib/call";
import { isFailure } from "@/lib/envelope";

export type ReadState =
  | { state: "idle" }
  | { state: "loading" }
  | { state: "ok"; text: string }
  | { state: "failed"; failure: Failure };

/** Reads a workspace file through `read_text`. A plain effect for now; the data layer
 *  can move to TanStack Query later without touching `Viewer`. A change of `root` or
 *  `rel` drops the old result at once, so no stale file shows. */
export function useReadText(root: string | null, rel: string | null): ReadState {
  const key = root !== null && rel !== null ? `${root}\0${rel}` : null;
  const [done, setDone] = useState<{ key: string; state: ReadState } | null>(null);

  useEffect(() => {
    if (key === null || root === null || rel === null) return;
    let live = true;
    call<string>("read_text", { root, rel }).then((e) => {
      if (!live) return;
      setDone({
        key,
        state: isFailure(e) ? { state: "failed", failure: e } : { state: "ok", text: e.data },
      });
    });
    return () => {
      live = false;
    };
  }, [key, root, rel]);

  if (key === null) return { state: "idle" };
  return done !== null && done.key === key ? done.state : { state: "loading" };
}
