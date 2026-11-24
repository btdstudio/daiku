// Amber lives here and in StatusBar.tsx and ListRail.tsx, nowhere else (S-0023 R12).
// Every other surface uses these constants or StatusText.

/** The next mark: a raised surface with a 3px amber left edge, on one row or card per
 *  surface. */
export const NEXT_MARK = "bg-surface-next border-l-[3px] border-l-attention";

/** The ink of a draft label (the Draft group and column headers, "draft" status text). */
export const DRAFT_INK = "text-attention-ink";
