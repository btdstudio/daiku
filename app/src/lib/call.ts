import { invoke } from "@tauri-apps/api/core";
import type { Envelope } from "@/lib/envelope";

/** A failure of the call itself, in the shape of a core `Failure`. */
export function internalFailure(err: unknown): Envelope<never> {
  return { schema: 1, error: { kind: "Internal", message: String(err) } };
}

/**
 * Invokes a command that returns an envelope. Always resolves: a rejection (the IPC
 * bridge, an unregistered command, a denied capability) becomes an `Internal` Failure.
 */
export async function call<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<Envelope<T>> {
  try {
    return await invoke<Envelope<T>>(cmd, args);
  } catch (err) {
    return internalFailure(err);
  }
}
