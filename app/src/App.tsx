import { invoke } from "@tauri-apps/api/core";
import { QueryClientProvider } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { queryClient } from "@/data/queryClient";
import { Frame } from "@/frame/Frame";
import { openRoot } from "@/frame/openWorkspace";
import { useViewState } from "@/frame/viewState";

export default function App() {
  // A measure build names its workspace; any other build returns null (design D7).
  // `undefined` while asking.
  const [measureRoot, setMeasureRoot] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    invoke<string | null>("measure_root")
      .then(setMeasureRoot)
      .catch(() => setMeasureRoot(null));
  }, []);
  if (measureRoot === undefined) return null;
  return <Window measureRoot={measureRoot} />;
}

/** The frame on view state. In a measure build the one workspace is opened without
 *  reading or saving view state (D16). */
function Window({ measureRoot }: { measureRoot: string | null }) {
  const { state, dispatch, ready } = useViewState(measureRoot === null);
  const selected = state.view.selected;
  const shown = selected !== null && state.refs[selected] !== undefined;

  useEffect(() => {
    if (measureRoot) void openRoot(measureRoot, dispatch);
  }, [measureRoot, dispatch]);

  // In measure mode, report once the frame has rendered with the workspace.
  useEffect(() => {
    if (!measureRoot || !shown) return;
    requestAnimationFrame(() => {
      invoke("measure_ready").catch(() => {});
    });
  }, [measureRoot, shown]);

  if (!ready) return null;
  return (
    <QueryClientProvider client={queryClient}>
      <Frame state={state} dispatch={dispatch} />
    </QueryClientProvider>
  );
}
