import { describe, expect, it } from "vitest";
import { splitFrontMatter } from "./splitFrontMatter";

describe("splitFrontMatter", () => {
  it("splits a closed leading block", () => {
    expect(splitFrontMatter("---\ntitle: x\nid: 1\n---\nbody\nmore")).toEqual({
      front: "title: x\nid: 1",
      body: "body\nmore",
    });
  });

  it("keeps an unterminated block as body", () => {
    const text = "---\ntitle: x\nbody";
    expect(splitFrontMatter(text)).toEqual({ front: null, body: text });
  });

  it("ignores a --- that isn't on the first line", () => {
    const text = "intro\n\n---\n\nafter";
    expect(splitFrontMatter(text)).toEqual({ front: null, body: text });
    const later = "\n---\na: b\n---\nx";
    expect(splitFrontMatter(later).front).toBeNull();
  });

  it("handles CRLF", () => {
    expect(splitFrontMatter("---\r\ntitle: x\r\n---\r\nbody")).toEqual({
      front: "title: x",
      body: "body",
    });
  });

  it("handles an empty block", () => {
    expect(splitFrontMatter("---\n---\nbody")).toEqual({ front: "", body: "body" });
  });

  it("is not fooled by a longer marker", () => {
    const text = "----\na\n---\nb";
    expect(splitFrontMatter(text).front).toBeNull();
  });
});
