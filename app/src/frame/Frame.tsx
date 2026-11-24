import type { ReactNode } from "react";
import { Board } from "@/board/Board";
import { BuildList } from "@/build/BuildList";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { TooltipProvider } from "@/components/ui/tooltip";
import { PlanList } from "@/plan/PlanList";
import { Viewer } from "@/viewer/Viewer";
import { gridColumns, useFold } from "./fold";
import { LaunchScreen } from "./LaunchScreen";
import { ListRail } from "./ListRail";
import { NotWorkspace } from "./NotWorkspace";
import { StatusBar } from "./StatusBar";
import { TitleBar } from "./TitleBar";
import type { Action, FrameState, Pane, WorkspaceRef } from "./viewState";
import { WorkspacesRail } from "./WorkspacesRail";

interface Props {
  state: FrameState;
  dispatch: (a: Action) => void;
}

/** The window: title bar, workspaces rail, the panes for the fold and focus, and the
 *  status bar. No terminal column (R10). */
export function Frame({ state, dispatch }: Props) {
  const fold = useFold();
  const { view, refs, notWorkspace } = state;
  const refOf = (root: string): WorkspaceRef | undefined => refs[root];
  const open = view.open.map(refOf).filter((r): r is WorkspaceRef => r !== undefined);
  const recents = view.recents.map(refOf).filter((r): r is WorkspaceRef => r !== undefined);
  const selected = view.selected ? (refOf(view.selected) ?? null) : null;

  const setFocus = (focus: typeof view.focus) => dispatch({ type: "focus", focus });
  const toBoard = () => setFocus("board");
  const toSplit = () => setFocus("split");
  const message = notWorkspace ? <NotWorkspace folder={notWorkspace} /> : null;

  const list =
    view.tab === "build" ? <BuildList onFill={toBoard} /> : <PlanList onFill={toBoard} />;
  const viewer = (
    <Viewer
      focused={view.focus === "viewer"}
      onFill={() => setFocus(view.focus === "viewer" ? "split" : "viewer")}
    />
  );

  let columns: string;
  let body: ReactNode;
  if (open.length === 0) {
    columns = gridColumns(fold, "viewer");
    body = (
      <div className="min-h-0 overflow-auto bg-surface-window">
        {message}
        <LaunchScreen recents={recents} dispatch={dispatch} />
      </div>
    );
  } else {
    columns = gridColumns(fold, view.focus);
    if (view.focus === "board") {
      body = message ?? <Board onRestore={toSplit} />;
    } else if (view.focus === "viewer") {
      body = message ?? viewer;
    } else if (fold === "wide") {
      body = (
        <>
          {message ?? viewer}
          {list}
        </>
      );
    } else if (fold === "folded") {
      body = (
        <>
          {message ?? viewer}
          <ListRail tab={view.tab}>{list}</ListRail>
        </>
      );
    } else {
      body = (
        <div className="flex min-h-0 min-w-0 flex-col">
          {message}
          <Tabs
            value={view.pane}
            onValueChange={(pane) => dispatch({ type: "pane", pane: pane as Pane })}
            className="min-h-0 flex-1 gap-0"
          >
            <TabsList className="m-2 self-center">
              <TabsTrigger value="viewer">Viewer</TabsTrigger>
              <TabsTrigger value="list">List</TabsTrigger>
            </TabsList>
            <TabsContent value="viewer" className="min-h-0">
              {viewer}
            </TabsContent>
            <TabsContent value="list" className="min-h-0">
              {list}
            </TabsContent>
          </Tabs>
        </div>
      );
    }
  }

  return (
    <TooltipProvider>
      <div className="grid h-screen grid-rows-[44px_1fr_30px] bg-surface-window">
        <TitleBar
          tab={view.tab}
          onTab={(tab) => dispatch({ type: "tab", tab })}
          focus={view.focus}
          onFocus={setFocus}
        />
        <div className="grid min-h-0" style={{ gridTemplateColumns: columns }}>
          <WorkspacesRail open={open} selected={view.selected} dispatch={dispatch} />
          {body}
        </div>
        <StatusBar fold={fold} name={selected?.name ?? null} tab={view.tab} />
      </div>
    </TooltipProvider>
  );
}
