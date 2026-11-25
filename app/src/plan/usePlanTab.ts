import { useMemo } from "react";
import { failureText } from "@/data/envelope";
import { useAdrs, useIdeas, useStatus } from "@/data/hooks";
import type { Record as Adr } from "@/generated/AdrRecord";
import type { Change } from "@/generated/Change";
import type { Idea } from "@/generated/Idea";

export interface PlanTab {
  /** Open plans, sorted by name. */
  plans: Change[];
  /** Every idea, by id. */
  ideas: Idea[];
  /** Newest number first. */
  adrs: Adr[];
  /** Each group's whole size, keyed by the rail's group name. */
  counts: Record<string, number>;
  /** Every warning from all three queries, status first; the data stays in its fields. */
  warnings: string[];
  /** The first failure of the three queries, as `failureText` gives it; the last data
   *  stays in the rest. */
  error: string | null;
  loading: boolean;
}

const NONE: never[] = [];
const EMPTY_WARNINGS: string[] = [];

/** The Plan tab's three queries joined: open plans, ideas and ADRs, ordered, with the
 *  rail's counts. A failed refetch keeps the last data, with `error` set. */
export function usePlanTab(root: string | null): PlanTab {
  const status = useStatus(root);
  const ideas = useIdeas(root);
  const adrs = useAdrs(root);
  const planData = status.data?.plans;
  const ideaData = ideas.data;
  const adrData = adrs.data;

  const joined = useMemo(() => {
    const plans = [...(planData ?? NONE)].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    const ideaRows = [...(ideaData ?? NONE)].sort((a, b) => a.id - b.id);
    const adrRows = [...(adrData ?? NONE)].sort((a, b) => b.number - a.number);
    return {
      plans,
      ideas: ideaRows,
      adrs: adrRows,
      counts: {
        Plans: plans.length,
        Ideas: ideaRows.length,
        ADRs: adrRows.length,
        References: 0,
      },
    };
  }, [planData, ideaData, adrData]);

  const err = status.error ?? ideas.error ?? adrs.error;
  const warnings =
    status.warnings.length > 0 || ideas.warnings.length > 0 || adrs.warnings.length > 0
      ? [...status.warnings, ...ideas.warnings, ...adrs.warnings]
      : EMPTY_WARNINGS;
  return {
    ...joined,
    warnings,
    error: err ? failureText(err) : null,
    loading: root !== null && (status.isPending || ideas.isPending || adrs.isPending) && !err,
  };
}
