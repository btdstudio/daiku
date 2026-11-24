import { Circle, CircleCheck, CircleDashed, Contrast, type LucideIcon } from "lucide-react";
import type { ListColumn } from "./groups";

/** The status mark of each list group (design.md, Screens/Build). */
export const MARKS: Record<ListColumn, LucideIcon> = {
  inBuild: Contrast,
  ready: Circle,
  draft: CircleDashed,
  done: CircleCheck,
};
