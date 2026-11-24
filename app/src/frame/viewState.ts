import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useReducer } from "react";
import type { Focus } from "./fold";

export type Tab = "plan" | "build";
export type Pane = "viewer" | "list";

/** What `view-state.json` holds (src-tauri/src/view_state.rs). Roots are raw. */
export interface ViewState {
  schema: number;
  open: string[];
  recents: string[];
  selected: string | null;
  tab: Tab;
  focus: Focus;
  pane: Pane;
  window: { width: number; height: number } | null;
}

/** A root and what is safe to show for it. Never show `root` itself (design D6). */
export interface WorkspaceRef {
  root: string;
  name: string;
  shown: string;
  isWorkspace: boolean;
}

export const RECENTS_CAP = 10;

export interface FrameState {
  view: ViewState;
  refs: Record<string, WorkspaceRef>;
  notWorkspace: WorkspaceRef | null;
  /** Counts changes worth saving; the save effect watches it. */
  changes: number;
}

export type Action =
  | { type: "loaded"; state: ViewState; refs: WorkspaceRef[] }
  | { type: "opened"; ref: WorkspaceRef }
  | { type: "notAWorkspace"; ref: WorkspaceRef }
  | { type: "selected"; root: string }
  | { type: "closed"; root: string }
  | { type: "tab"; tab: Tab }
  | { type: "focus"; focus: Focus }
  | { type: "pane"; pane: Pane };

export const defaultView: ViewState = {
  schema: 1,
  open: [],
  recents: [],
  selected: null,
  tab: "build",
  focus: "split",
  pane: "viewer",
  window: null,
};

export const initialState: FrameState = {
  view: defaultView,
  refs: {},
  notWorkspace: null,
  changes: 0,
};

function changed(s: FrameState, view: ViewState, extra: Partial<FrameState> = {}): FrameState {
  return { ...s, ...extra, view, changes: s.changes + 1 };
}

export function reducer(s: FrameState, a: Action): FrameState {
  switch (a.type) {
    case "loaded": {
      const refs: Record<string, WorkspaceRef> = {};
      for (const r of a.refs) refs[r.root] = r;
      return { ...s, view: { ...defaultView, ...a.state }, refs, notWorkspace: null };
    }
    case "opened": {
      const { root } = a.ref;
      const refs = { ...s.refs, [root]: a.ref };
      if (s.view.open.includes(root)) {
        return changed(s, { ...s.view, selected: root }, { refs, notWorkspace: null });
      }
      const recents = [root, ...s.view.recents.filter((r) => r !== root)].slice(0, RECENTS_CAP);
      const view = { ...s.view, open: [...s.view.open, root], recents, selected: root };
      return changed(s, view, { refs, notWorkspace: null });
    }
    case "notAWorkspace":
      return { ...s, notWorkspace: a.ref };
    case "selected":
      if (!s.view.open.includes(a.root)) return s;
      return changed(s, { ...s.view, selected: a.root }, { notWorkspace: null });
    case "closed": {
      const i = s.view.open.indexOf(a.root);
      if (i < 0) return s;
      const open = s.view.open.filter((r) => r !== a.root);
      const selected =
        s.view.selected === a.root ? (open[i] ?? open[i - 1] ?? null) : s.view.selected;
      return changed(s, { ...s.view, open, selected });
    }
    case "tab":
      return changed(s, { ...s.view, tab: a.tab });
    case "focus":
      return changed(s, { ...s.view, focus: a.focus });
    case "pane":
      return changed(s, { ...s.view, pane: a.pane });
  }
}

/**
 * The frame's state: loads `view-state.json` once and, when `persist` is on, saves
 * on every change. Changes are clicks, so there is nothing to debounce, and a
 * debounced save could be lost at quit. A rejected load leaves the defaults.
 */
export function useViewState(persist = true) {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [ready, markReady] = useReducer(() => true, false);

  useEffect(() => {
    if (!persist) {
      markReady();
      return;
    }
    let live = true;
    invoke<{ state: ViewState; refs: WorkspaceRef[] }>("view_state_load")
      .then((r) => {
        if (live) dispatch({ type: "loaded", state: r.state, refs: r.refs });
      })
      .catch((err) => console.error("view_state_load failed", err))
      .finally(() => {
        if (live) markReady();
      });
    return () => {
      live = false;
    };
  }, [persist]);

  useEffect(() => {
    if (!persist || state.changes === 0) return;
    invoke("view_state_save", { state: state.view }).catch((err) =>
      console.error("view_state_save failed", err),
    );
  }, [persist, state.changes, state.view]);

  const stable = useCallback((a: Action) => dispatch(a), []);
  return { state, dispatch: stable, ready };
}
