import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { FailureError } from "@/data/envelope";
import { QueryFeedback } from "./QueryFeedback";

const failed = (kind: string, message: string) =>
  new FailureError({ schema: 1, error: { kind, message } });

describe("QueryFeedback", () => {
  afterEach(cleanup);

  it("shows each warning as its own text, in the envelope's order", () => {
    render(<QueryFeedback warnings={["first warning", "second warning"]} />);
    expect(screen.getByText("first warning")).toBeTruthy();
    expect(screen.getByText("second warning")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows an error as Kind: message, beside the warnings", () => {
    render(<QueryFeedback warnings={["kept warning"]} error={failed("Io", "disk went away")} />);
    expect(screen.getByRole("alert").textContent).toBe("Io: disk went away");
    expect(screen.getByText("kept warning")).toBeTruthy();
  });

  it("renders nothing for an empty warning array and no error", () => {
    const { container } = render(<QueryFeedback warnings={[]} />);
    expect(container.innerHTML).toBe("");
  });

  it("puts the inline variant on one line", () => {
    const { container } = render(
      <QueryFeedback inline warnings={["one", "two"]} error={failed("Syntax", "truncated")} />,
    );
    expect(container.querySelectorAll("p")).toHaveLength(0);
    expect(container.textContent).toBe("one · two · Syntax: truncated");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("renders nothing inline when there is no feedback", () => {
    const { container } = render(<QueryFeedback inline warnings={[]} />);
    expect(container.innerHTML).toBe("");
  });
});
