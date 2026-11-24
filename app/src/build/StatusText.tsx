import { DRAFT_INK } from "@/frame/marks";

/** A ticket's status as text; a draft reads in the draft ink (S-0023 D10). */
export function StatusText({ status }: { status: string }) {
  return <span className={status === "draft" ? DRAFT_INK : undefined}>{status}</span>;
}
