import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderUi } from "../../../test/render";

import { Disclosure } from "./Disclosure";

describe("Disclosure", () => {
  it("keeps the detail one click away", () => {
    const { container } = renderUi(
      <Disclosure summary="What Campbooks keeps">Your name and address.</Disclosure>,
    );
    const details = container.querySelector("details");

    expect(details).not.toHaveAttribute("open");
    fireEvent.click(screen.getByText("What Campbooks keeps"));
    expect(details).toHaveAttribute("open");
  });

  it("can start open", () => {
    const { container } = renderUi(
      <Disclosure summary="Why" defaultOpen>
        Because.
      </Disclosure>,
    );

    expect(container.querySelector("details")).toHaveAttribute("open");
  });
});
