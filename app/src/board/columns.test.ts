import { describe, expect, it } from "vitest";
import { buildIndex, columnFor } from "./columns";
import { build, status, ticket } from "./fixtures";

const none = new Map();
const noClosed = new Set<string>();

describe("columnFor", () => {
  it("puts a closed build first, ahead of a shipped one (rule 1)", () => {
    const s = status([{ repo: "daiku", builds: [build("S-1", { gates: { ship: "complete" } })] }], ["S-1"]);
    expect(columnFor(ticket("S-1"), buildIndex(s), new Set(s.closedBuilds))).toBe("closed");
  });

  it("puts an open build with ship complete in done (rule 2)", () => {
    const s = status([{ repo: "daiku", builds: [build("S-1", { gates: { ship: "complete" } })] }]);
    expect(columnFor(ticket("S-1", { status: "ready" }), buildIndex(s), noClosed)).toBe("done");
  });

  it("puts any other open build in inBuild, whatever the ticket's status (rule 3)", () => {
    const s = status([{ repo: "daiku", builds: [build("S-1", { gates: { ship: "in-progress" } })] }]);
    const idx = buildIndex(s);
    expect(columnFor(ticket("S-1", { status: "published" }), idx, noClosed)).toBe("inBuild");
    expect(columnFor(ticket("S-1", { status: "draft" }), idx, noClosed)).toBe("inBuild");
  });

  it("puts a ticket with no build and ready or published status in ready (rule 4)", () => {
    expect(columnFor(ticket("S-1", { status: "ready" }), none, noClosed)).toBe("ready");
    expect(columnFor(ticket("S-1", { status: "published" }), none, noClosed)).toBe("ready");
  });

  it("puts a ticket with no build and draft status in draft (rule 5)", () => {
    expect(columnFor(ticket("S-1", { status: "draft" }), none, noClosed)).toBe("draft");
  });

  it("puts a no-build ticket with another status in no column (rule 6)", () => {
    expect(columnFor(ticket("S-1", { status: "closed" }), none, noClosed)).toBeNull();
    expect(columnFor(ticket("S-1", { status: "weird" }), none, noClosed)).toBeNull();
  });

  it("places an inbox ticket by its build", () => {
    const s = status([{ repo: "api", builds: [build("N-0001", { repos: ["api"] })] }]);
    const t = ticket("N-0001", { inbox: true, change: "", repo: "api" });
    expect(columnFor(t, buildIndex(s), noClosed)).toBe("inBuild");
  });

  it("places a ticket id with no build by its status", () => {
    const s = status([{ repo: "daiku", builds: [build("S-9")] }]);
    expect(columnFor(ticket("S-1", { status: "ready" }), buildIndex(s), noClosed)).toBe("ready");
  });
});

describe("buildIndex", () => {
  it("records the repo and whether the build holds its repo's lock", () => {
    const s = status([
      { repo: "daiku", holder: "S-1", builds: [build("S-1"), build("S-2")] },
      { repo: "api", builds: [build("N-1", { repos: ["api"] })] },
    ]);
    const idx = buildIndex(s);
    expect(idx.get("S-1")).toMatchObject({ repo: "daiku", holdsLock: true });
    expect(idx.get("S-2")).toMatchObject({ repo: "daiku", holdsLock: false });
    expect(idx.get("N-1")).toMatchObject({ repo: "api", holdsLock: false });
  });

  it("lets the first build with an id win", () => {
    const s = status([
      { repo: "a", builds: [build("S-1", { phase: "design" })] },
      { repo: "b", builds: [build("S-1", { phase: "test" })] },
    ]);
    const got = buildIndex(s).get("S-1");
    expect(got?.build.phase).toBe("design");
    expect(got?.repo).toBe("a");
  });
});
