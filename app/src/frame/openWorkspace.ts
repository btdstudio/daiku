import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Action, WorkspaceRef } from "./viewState";

/**
 * Probes `root` and opens it as a workspace, or records that it isn't one. A
 * rejection shows nothing and is logged; no raw root reaches the screen (design D6).
 */
export async function openRoot(root: string, dispatch: (a: Action) => void): Promise<void> {
  try {
    const ref = await invoke<WorkspaceRef>("probe_workspace", { root });
    dispatch(ref.isWorkspace ? { type: "opened", ref } : { type: "notAWorkspace", ref });
  } catch (err) {
    console.error("probe_workspace failed", err);
  }
}

/** The system folder picker, then [`openRoot`]. */
export async function pickAndOpen(dispatch: (a: Action) => void): Promise<void> {
  try {
    const chosen = await open({ directory: true });
    if (typeof chosen === "string") await openRoot(chosen, dispatch);
  } catch (err) {
    console.error("folder picker failed", err);
  }
}
