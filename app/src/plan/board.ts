import type { Change } from "@/generated/Change";

/** The Plan board column of an open plan: its `phase`, or the first phase when that
 *  is empty (no gate yet). */
export function planColumn(plan: Change, phases: string[]): string {
  return plan.phase || phases[0];
}
