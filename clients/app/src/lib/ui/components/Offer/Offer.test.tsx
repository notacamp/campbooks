import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../../../test/render";
import { Progress } from "../Progress/Progress";

import { Offer } from "./Offer";

describe("Offer and Progress", () => {
  it("asks one question as a non-modal dialog with its actions", () => {
    renderUi(
      <Offer
        title="Move all 12 like this to Updates?"
        detail="None of them looks suspicious."
        actions={<button type="button">Move all 12</button>}
        foot="You can undo it."
      >
        <p>a preview</p>
      </Offer>,
    );
    const dialog = screen.getByRole("dialog", { name: "Move all 12 like this to Updates?" });
    expect(dialog).toHaveAttribute("aria-modal", "false");
    expect(dialog).toHaveTextContent("None of them looks suspicious.");
    expect(dialog).toHaveTextContent("a preview");
    expect(screen.getByRole("button", { name: "Move all 12" })).toBeInTheDocument();
  });

  it("says how far the work has got", () => {
    renderUi(
      <Progress label="Moving 1,000" value={340} max={1000}>
        340 of 1,000 done
      </Progress>,
    );
    const bar = screen.getByRole("progressbar", { name: "Moving 1,000" });
    expect(bar).toHaveAttribute("aria-valuenow", "340");
    expect(bar).toHaveAttribute("aria-valuemax", "1000");
    expect(screen.getByText("340 of 1,000 done")).toBeInTheDocument();
  });

  it("claims no figure while the total is unknown, and announces its line", () => {
    renderUi(
      <Progress label="Finding them" value={0} max={0}>
        Counting…
      </Progress>,
    );
    const bar = screen.getByRole("progressbar", { name: "Finding them" });
    expect(bar).not.toHaveAttribute("aria-valuenow");
    expect(bar).not.toHaveAttribute("aria-valuemax");
    expect(screen.getByText("Counting…")).toHaveAttribute("aria-live", "polite");
  });

  it("never runs past the end", () => {
    renderUi(<Progress label="Putting back" value={12} max={10} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "10");
  });
});
