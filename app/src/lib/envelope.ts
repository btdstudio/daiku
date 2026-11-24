import type { Failure } from "@/generated/Failure";

/** A successful read: the `--json` envelope around `T`. */
export interface Success<T> {
  schema: number;
  data: T;
  warnings: string[];
}

export type Envelope<T> = Success<T> | Failure;

export function isFailure<T>(e: Envelope<T>): e is Failure {
  return "error" in e;
}
