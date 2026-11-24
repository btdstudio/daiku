import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StatusBar } from "./StatusBar";

afterEach(cleanup);

describe("StatusBar", () => {
  it("shows the active change and its phase beside the workspace and tab", () => {
    const { container } = render(
      <StatusBar
        fold="wide"
        name="ws"
        tab="build"
        active={{ name: "S-0023", phase: "implement" }}
        nudge={null}
      />,
    );
    expect(container.textContent).toBe("ws · Build · S-0023 implement");
  });

  it("keeps the name-only tab rule below wide, and shows the active change", () => {
    const { container } = render(
      <StatusBar fold="folded" name="ws" tab="plan" active={{ name: "4-x", phase: "spec" }} nudge={null} />,
    );
    expect(container.textContent).toBe("ws · 4-x spec");
  });

  it("shows nothing extra without an active change", () => {
    const { container } = render(<StatusBar fold="wide" name="ws" tab="plan" nudge={null} />);
    expect(container.textContent).toBe("ws · Plan");
    expect(container.querySelector('[data-slot="active"]')).toBeNull();
  });

  it("renders the nudge as an amber button with its text as the title", () => {
    const onNudge = vi.fn();
    render(<StatusBar fold="wide" name="ws" tab="build" nudge="Next: N-1 is a draft." onNudge={onNudge} />);
    const b = screen.getByRole("button", { name: "Next: N-1 is a draft." });
    expect(b.getAttribute("title")).toBe("Next: N-1 is a draft.");
    expect(b.className).toContain("text-attention-ink");
    expect(b.className).toContain("truncate");
    fireEvent.click(b);
    expect(onNudge).toHaveBeenCalledOnce();
  });

  it("shows text without a button when it is not clickable, and nothing for null", () => {
    const { rerender } = render(
      <StatusBar fold="wide" name="ws" tab="build" nudge="Nothing is waiting on you" />,
    );
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("Nothing is waiting on you")).toBeTruthy();
    rerender(<StatusBar fold="wide" name="ws" tab="build" nudge={null} />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(document.querySelector('[data-slot="nudge"]')).toBeNull();
  });

  it("renders a control-character id as text, with no HTML", () => {
    const text = "Next: <img src=x onerror=1> \\x1b[0m";
    const { container } = render(
      <StatusBar fold="wide" name="ws" tab="build" nudge={text} onNudge={() => {}} />,
    );
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByRole("button").textContent).toBe(text);
  });
});
