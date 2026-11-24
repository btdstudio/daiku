// Test builders for tickets, builds and status.
import type { Change } from "@/generated/Change";
import type { Status } from "@/generated/Status";
import type { TicketView } from "@/generated/TicketView";

export function ticket(id: string, over: Partial<TicketView> = {}): TicketView {
  return {
    body: "",
    change: "4-plan",
    changeClosed: false,
    dependsOn: [],
    drifted: false,
    external: {},
    frozen: false,
    id,
    inbox: false,
    labels: [],
    parent: "",
    path: `plan/specs/4-plan/tickets/${id}.md`,
    repo: "daiku",
    requirements: [],
    status: "draft",
    title: `Title ${id}`,
    type: "story",
    ...over,
  };
}

export function build(name: string, over: Partial<Change> = {}): Change {
  return {
    gates: {},
    loop: "build",
    name,
    phase: "implement",
    reasons: {},
    repos: ["daiku"],
    sources: [],
    status: "in-progress",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...over,
  };
}

export function status(
  builds: { repo: string; holder?: string | null; builds: Change[] }[],
  closedBuilds: string[] = [],
): Status {
  return {
    vcs: "git",
    active: null,
    noChanges: false,
    plans: [],
    builds: builds.map((b) => ({ repo: b.repo, holder: b.holder ?? null, builds: b.builds })),
    closedBuilds,
  };
}


/** A success envelope. */
export const ok = (data: unknown) => ({ schema: 1, data, warnings: [] });
/** A Failure envelope. */
export const fail = (kind: string, message: string) => ({ schema: 1, error: { kind, message } });
