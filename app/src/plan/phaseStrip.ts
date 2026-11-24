import type { ArtifactList } from "@/generated/ArtifactList";

export const NOT_STARTED = "not started";

export interface StripPhase {
  name: string;
  /** `change.gates[name]`, or "not started" when the phase has no gate yet. */
  status: string;
  skipped: boolean;
  /** `change.reasons[name]`; empty unless the phase has one (a skipped phase). */
  reason: string;
  /** The change's `phase`, or the first phase when that is empty. */
  current: boolean;
  /** The paths of the phase's files, in `artifacts` order (as the envelope shows them). */
  files: string[];
}

/** Every phase of a plan in order, with its gate status, skip reason and files.
 *  `phases` defaults to the list's own (the machine's names); the Plan tab passes
 *  `Status.planPhases`. */
export function phaseStrip(list: ArtifactList, phases: string[] = list.phases): StripPhase[] {
  const current = list.change.phase || phases[0];
  return phases.map((name) => {
    const status = list.change.gates[name] ?? NOT_STARTED;
    return {
      name,
      status,
      skipped: status === "skipped",
      reason: list.change.reasons[name] ?? "",
      current: name === current,
      files: list.artifacts.filter((a) => a.phase === name).map((a) => a.path),
    };
  });
}
