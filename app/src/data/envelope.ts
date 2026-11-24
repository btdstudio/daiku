import type { Failure } from "@/generated/Failure";
import { type Envelope, isFailure } from "@/lib/envelope";

/** A Failure envelope, thrown so a query errors with it. */
export class FailureError extends Error {
  readonly failure: Failure;
  constructor(failure: Failure) {
    super(failure.error.message);
    this.name = "FailureError";
    this.failure = failure;
  }
}

/** The envelope's data, or a thrown `FailureError`. */
export function unwrap<T>(e: Envelope<T>): T {
  if (isFailure(e)) throw new FailureError(e);
  return e.data;
}
