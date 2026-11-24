import { useEffect, useState } from "react";

/** How much of the frame fits: three columns, a folded list rail, or one pane (D9). */
export type Fold = "wide" | "folded" | "narrow";
export type Focus = "split" | "viewer" | "board";

export const WIDE_AT = 1440;
export const FOLDED_AT = 1000;

export function foldFor(width: number): Fold {
  if (width >= WIDE_AT) return "wide";
  if (width >= FOLDED_AT) return "folded";
  return "narrow";
}

/** The grid's column template for a fold and a focus (design D9's table). */
export function gridColumns(fold: Fold, focus: Focus): string {
  if (focus !== "split") return "52px 1fr";
  if (fold === "wide") return "52px 1fr 340px";
  if (fold === "folded") return "52px 1fr 52px";
  return "52px 1fr";
}

/** The window's fold, kept current on resize. */
export function useFold(): Fold {
  const [fold, setFold] = useState<Fold>(() => foldFor(window.innerWidth));
  useEffect(() => {
    const onResize = () => setFold(foldFor(window.innerWidth));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return fold;
}
