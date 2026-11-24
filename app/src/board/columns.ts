import type { Change } from "@/generated/Change";
import type { Status } from "@/generated/Status";
import type { TicketView } from "@/generated/TicketView";

export type Column = "inBuild" | "ready" | "draft" | "done" | "closed";

/** An open build joined to its ticket, with the repo it's in and whether it holds
 *  that repo's lock. */
export interface BuildInfo {
  build: Change;
  repo: string;
  holdsLock: boolean;
}

/** Open builds by name (a build is named by its ticket id). A duplicate id shares
 *  the first build. */
export function buildIndex(status: Status): Map<string, BuildInfo> {
  const out = new Map<string, BuildInfo>();
  for (const group of status.builds) {
    for (const build of group.builds) {
      if (out.has(build.name)) continue;
      out.set(build.name, {
        build,
        repo: group.repo,
        holdsLock: group.holder === build.name,
      });
    }
  }
  return out;
}

/** The column a ticket sits in, or null for none. Builds beat the ticket's own
 *  status, which isn't updated when a build ships (spec R10, R11). */
export function columnFor(
  t: TicketView,
  builds: Map<string, BuildInfo>,
  closed: Set<string>,
): Column | null {
  if (closed.has(t.id)) return "closed";
  const info = builds.get(t.id);
  if (info) return info.build.gates["ship"] === "complete" ? "done" : "inBuild";
  if (t.status === "ready" || t.status === "published") return "ready";
  if (t.status === "draft") return "draft";
  return null;
}
