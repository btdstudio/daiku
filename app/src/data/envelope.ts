import type { Failure } from "@/generated/Failure";
import { type Envelope, isFailure, type Success } from "@/lib/envelope";

/** A Failure envelope, thrown so a query errors with it. */
export class FailureError extends Error {
  readonly failure: Failure;
  constructor(failure: Failure) {
    super(failure.error.message);
    this.name = "FailureError";
    this.failure = failure;
  }
}

/** The `Success<T>` envelope, or a thrown `FailureError`: a query keeps the warnings it
 *  came with (`S-0019`). */
export function envelope<T>(e: Envelope<T>): Success<T> {
  if (isFailure(e)) throw new FailureError(e);
  return e;
}

/** "File too large to show (N MiB)" for the `TooLarge` Failure, from its message. */
function tooLarge(message: string): string {
  const n = /\(([\d.]+) MiB\)/.exec(message)?.[1];
  return n ? `File too large to show (${n} MiB)` : "File too large to show";
}

/** A query's failure as text: `TooLarge` reworded, others as "Kind: message". */
export function failureText(e: unknown): string {
  if (!(e instanceof FailureError)) return String(e);
  const { kind, message } = e.failure.error;
  return kind === "TooLarge" ? tooLarge(message) : `${kind}: ${message}`;
}
