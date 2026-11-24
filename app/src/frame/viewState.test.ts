import { describe, expect, it } from "vitest";
import { type Action, type FrameState, initialState, reducer, type WorkspaceRef } from "./viewState";

const ref = (name: string): WorkspaceRef => ({
  root: `/w/${name}`,
  name,
  shown: `/w/${name}`,
  isWorkspace: true,
});

const run = (actions: Action[], from: FrameState = initialState) => actions.reduce(reducer, from);
const opened = (name: string): Action => ({ type: "opened", ref: ref(name) });

describe("reducer", () => {
  it("opened adds a chip, selects it and puts it first in recents", () => {
    const s = run([opened("a"), opened("b")]);
    expect(s.view.open).toEqual(["/w/a", "/w/b"]);
    expect(s.view.selected).toBe("/w/b");
    expect(s.view.recents).toEqual(["/w/b", "/w/a"]);
    expect(s.refs["/w/b"].name).toBe("b");
  });

  it("opened on an open root selects it without duplicating", () => {
    const s = run([opened("a"), opened("b"), opened("a")]);
    expect(s.view.open).toEqual(["/w/a", "/w/b"]);
    expect(s.view.selected).toBe("/w/a");
    expect(s.view.recents).toEqual(["/w/b", "/w/a"]);
  });

  it("caps recents at 10, most recent first", () => {
    const names = Array.from({ length: 12 }, (_, i) => `w${i}`);
    const s = run(names.map(opened));
    expect(s.view.recents).toHaveLength(10);
    expect(s.view.recents[0]).toBe("/w/w11");
    expect(s.view.recents[9]).toBe("/w/w2");
  });

  it("closed selects the right neighbour, else the left, else none; recents stay", () => {
    let s = run([opened("a"), opened("b"), opened("c"), { type: "selected", root: "/w/b" }]);
    s = reducer(s, { type: "closed", root: "/w/b" });
    expect(s.view.open).toEqual(["/w/a", "/w/c"]);
    expect(s.view.selected).toBe("/w/c");
    s = reducer(s, { type: "closed", root: "/w/c" });
    expect(s.view.selected).toBe("/w/a");
    s = reducer(s, { type: "closed", root: "/w/a" });
    expect(s.view.open).toEqual([]);
    expect(s.view.selected).toBeNull();
    expect(s.view.recents).toHaveLength(3);
  });

  it("closing an unselected chip keeps the selection", () => {
    const s = run([opened("a"), opened("b"), { type: "closed", root: "/w/a" }]);
    expect(s.view.selected).toBe("/w/b");
  });

  it("notAWorkspace adds no chip and no change to save", () => {
    const s = run([opened("a"), { type: "notAWorkspace", ref: { ...ref("x"), isWorkspace: false } }]);
    expect(s.view.open).toEqual(["/w/a"]);
    expect(s.notWorkspace?.name).toBe("x");
    expect(s.changes).toBe(1);
  });

  it("selecting or opening clears the message", () => {
    const bad: Action = { type: "notAWorkspace", ref: { ...ref("x"), isWorkspace: false } };
    expect(run([opened("a"), bad, { type: "selected", root: "/w/a" }]).notWorkspace).toBeNull();
    expect(run([bad, opened("a")]).notWorkspace).toBeNull();
  });

  it("tab, focus and pane set their field", () => {
    const s = run([
      { type: "tab", tab: "plan" },
      { type: "focus", focus: "board" },
      { type: "pane", pane: "list" },
    ]);
    expect(s.view.tab).toBe("plan");
    expect(s.view.focus).toBe("board");
    expect(s.view.pane).toBe("list");
  });

  it("loaded replaces the view without counting as a change", () => {
    const s = reducer(initialState, {
      type: "loaded",
      state: { ...initialState.view, open: ["/w/a"], selected: "/w/a" },
      refs: [ref("a")],
    });
    expect(s.view.selected).toBe("/w/a");
    expect(s.refs["/w/a"].name).toBe("a");
    expect(s.changes).toBe(0);
  });
});
