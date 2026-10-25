import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { ChoiceCards } from "./ChoiceCards";

describe("ChoiceCards", () => {
  it("is a named radio group that reports the choice", () => {
    const onChange = vi.fn();
    renderUi(
      <ChoiceCards
        label="What is it for?"
        hideLabel
        value="business"
        onChange={onChange}
        options={[
          { value: "business", title: "Business", description: "Clients and invoices." },
          { value: "personal", title: "Personal" },
        ]}
      />,
    );

    expect(screen.getByRole("group", { name: "What is it for?" })).toBeInTheDocument();
    const business = screen.getByRole("radio", { name: "Business" });
    expect(business).toBeChecked();
    expect(business).toHaveAccessibleDescription("Clients and invoices.");

    fireEvent.click(screen.getByRole("radio", { name: "Personal" }));

    expect(onChange).toHaveBeenCalledWith("personal");
  });

  it("describes a card by its tag and its sentence, and names it by its title alone", () => {
    renderUi(
      <ChoiceCards
        label="What should happen?"
        value={null}
        onChange={vi.fn()}
        options={[
          {
            value: "guess",
            title: "Archive them",
            tag: "Scout’s guess",
            description: "Out of the way.",
          },
          { value: "plain", title: "Mark them read" },
        ]}
      />,
    );

    const guess = screen.getByRole("radio", { name: "Archive them" });
    expect(guess).toHaveAccessibleDescription("Scout’s guess Out of the way.");
    expect(screen.getByRole("radio", { name: "Mark them read" })).not.toHaveAccessibleDescription();
  });
});
