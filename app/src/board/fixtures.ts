// Test builders for tickets, builds and status.
import type { Record as Adr } from "@/generated/AdrRecord";
import type { Change } from "@/generated/Change";
import type { Idea } from "@/generated/Idea";
import type { Status } from "@/generated/Status";
import type { TicketView } from "@/generated/TicketView";

/** The plan machine's phase names, as `Status.planPhases` carries them. */
export const PLAN_PHASES = [
  "intake",
  "research",
  "explore",
  "synthesis",
  "spec",
  "architecture",
  "breakdown",
  "publish",
];

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

/** A status for the Plan tab: open plans, with the machine's phase names. */
export function planStatus(plans: Change[]): Status {
  return { ...status([]), plans };
}

export function idea(id: number, over: Partial<Idea> = {}): Idea {
  return {
    body: "",
    changes: [],
    dir: `ideas/${id}-idea`,
    id,
    reason: "",
    slug: "idea",
    status: "open",
    title: `Idea ${id}`,
    updated: "2026-01-02",
    ...over,
  };
}

export function adr(number: number, over: Partial<Adr> = {}): Adr {
  return {
    date: "2026-01-03",
    deciders: "",
    file: `${String(number).padStart(4, "0")}-adr.md`,
    number,
    path: `plan/04_architecture/adr/${String(number).padStart(4, "0")}-adr.md`,
    raw: "",
    slug: "adr",
    status: "accepted",
    title: `Decision ${number}`,
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
    planPhases: PLAN_PHASES,
  };
}


/** A success envelope. */
export const ok = (data: unknown) => ({ schema: 1, data, warnings: [] });
/** A Failure envelope. */
export const fail = (kind: string, message: string) => ({ schema: 1, error: { kind, message } });

export function plan(name: string, over: Partial<Change> = {}): Change {
  return {
    gates: {},
    loop: "plan",
    name,
    phase: "",
    reasons: {},
    repos: [],
    sources: [],
    status: "in-progress",
    updatedAt: "2026-01-01T00:00:00.000Z",
    ...over,
  };
}
