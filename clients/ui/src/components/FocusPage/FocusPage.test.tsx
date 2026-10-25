import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { Brand, FocusPage } from "./FocusPage";

describe("FocusPage", () => {
  it("asks one question as the page heading", () => {
    renderUi(
      <FocusPage
        top={<Brand name="Campbooks" />}
        eyebrow="Step 1 of 2"
        title="What should we call it?"
        lead="Any name works."
        footer="Help"
        accent="personal"
        actions={<button type="button">Continue</button>}
      />,
    );

    expect(
      screen.getByRole("heading", { level: 1, name: "What should we call it?" }),
    ).toBeVisible();
    expect(screen.getByRole("main")).toBeInTheDocument();
    expect(screen.getByText("Campbooks")).toBeInTheDocument();
    expect(screen.getByRole("main").parentElement).toHaveAttribute("data-accent", "personal");
  });

  it("submits its form on Enter without reloading", () => {
    const onSubmit = vi.fn();
    renderUi(
      <FocusPage title="Name it" onSubmit={onSubmit}>
        <input aria-label="Name" />
      </FocusPage>,
    );

    fireEvent.submit(screen.getByRole("form", { name: "Name it" }));

    expect(onSubmit).toHaveBeenCalledOnce();
  });
});
