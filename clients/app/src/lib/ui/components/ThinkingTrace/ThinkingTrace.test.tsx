import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";

import { ThinkingTrace } from "./ThinkingTrace";

const props = {
  label: "Scout is thinking…",
  summary: "How Scout thought about it",
};

describe("ThinkingTrace", () => {
  it("announces only its status while the thinking grows", () => {
    renderUi(<ThinkingTrace {...props} text="Marta asks about Thursday." live />);

    expect(screen.getByRole("status")).toHaveTextContent("Scout is thinking…");
    expect(screen.getByText("Marta asks about Thursday.")).toHaveAttribute("aria-live", "off");
  });

  it("shows the status alone before the first thought", () => {
    const { container } = renderUi(<ThinkingTrace {...props} text="" live />);

    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(container.querySelector("[aria-live='off']")).toBeNull();
  });

  it("folds away once the answer is written", () => {
    const { container } = renderUi(
      <ThinkingTrace {...props} text="Marta asks about Thursday." live={false} />,
    );
    const details = container.querySelector("details");

    expect(screen.queryByRole("status")).toBeNull();
    expect(details).not.toHaveAttribute("open");
    fireEvent.click(screen.getByText("How Scout thought about it"));
    expect(details).toHaveAttribute("open");
  });

  it("leaves nothing behind when there was no thinking", () => {
    const { container } = renderUi(<ThinkingTrace {...props} text="" live={false} />);

    expect(container.querySelector("details, [role='status']")).toBeNull();
  });
});
