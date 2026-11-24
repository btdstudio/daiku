import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FrontMatter } from "./FrontMatter";
import { Markdown } from "./Markdown";

const openUrl = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }));

afterEach(() => {
  cleanup();
  openUrl.mockClear();
});

const GFM = "| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n\n```ts\nconst x = 1;\n```\n";

describe("Markdown", () => {
  it("renders a table, task list and unhighlighted code (AC1)", () => {
    const { container } = render(<Markdown source={GFM} />);
    expect(container.querySelector("table")).not.toBeNull();
    const boxes = container.querySelectorAll('input[type="checkbox"]');
    expect(boxes).toHaveLength(2);
    boxes.forEach((b) => expect((b as HTMLInputElement).disabled).toBe(true));
    const code = container.querySelector("pre > code");
    expect(code?.textContent).toContain("const x = 1;");
    expect(code?.className).toContain("font-mono");
    expect(code?.querySelector("span")).toBeNull();
  });

  it("drops raw HTML (AC2)", () => {
    const { container } = render(
      <Markdown source={"<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\ntext"} />,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("[onerror]")).toBeNull();
  });

  it("links only https (AC5)", () => {
    const source = [
      "[a](https://example.com/a)",
      "[b](http://example.com/b)",
      "[c](file:///etc/passwd)",
      "[d](javascript:alert(1))",
      "[e](./rel.md)",
      "[f](mailto:a@b)",
      "[g](data:text/html,x)",
    ].join("\n\n");
    const { container } = render(<Markdown source={source} />);
    const links = container.querySelectorAll("a");
    expect(links).toHaveLength(1);
    expect(links[0].getAttribute("href")).toBe("https://example.com/a");
    for (const t of ["b", "c", "d", "e", "f", "g"]) expect(container.textContent).toContain(t);
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    links[0].dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    expect(openUrl).toHaveBeenCalledTimes(1);
    expect(openUrl).toHaveBeenCalledWith("https://example.com/a");
  });

  it("shows an image as its alt text and loads nothing (AC5)", () => {
    const { container } = render(<Markdown source="![alt text](https://x/y.png)" />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("alt text");
  });

  it("holds all at once (AC10)", () => {
    const { container } = render(
      <Markdown source={`${GFM}\n<script>alert(1)</script>\n\nbell \\x1b[31m here\n`} />,
    );
    expect(container.querySelector("table")).not.toBeNull();
    expect(container.querySelector("pre > code")).not.toBeNull();
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("\\x1b[31m");
  });

  it("renders standalone, with the front-matter block (AC9)", () => {
    const { container } = render(
      <>
        <FrontMatter text={"title: x"} />
        <Markdown source="# Head" />
      </>,
    );
    expect(container.querySelector("pre")?.textContent).toBe("title: x");
    expect(container.querySelector("h1")?.textContent).toBe("Head");
  });

  it("an in-page link doesn't navigate", () => {
    const { container } = render(<Markdown source="[top](#top)" />);
    const a = container.querySelector("a") as HTMLAnchorElement;
    expect(fireEvent.click(a)).toBe(false);
    expect(openUrl).not.toHaveBeenCalled();
  });
});
