import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

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
});
