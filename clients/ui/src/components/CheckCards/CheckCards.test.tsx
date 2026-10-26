import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { CheckCards } from "./CheckCards";

describe("CheckCards", () => {
  it("is a named group of checkboxes that reports every choice", () => {
    const onChange = vi.fn();
    renderUi(
      <CheckCards
        label="Whose mail should always reach you?"
        value={["rita"]}
        onChange={onChange}
        options={[
          {
            value: "rita",
            title: "Rita Gomes",
            description: "rita@example.com",
            meta: "41 emails",
          },
          { value: "marta", title: "Marta Ferreira" },
        ]}
      />,
    );

    expect(
      screen.getByRole("group", { name: "Whose mail should always reach you?" }),
    ).toBeInTheDocument();
    const rita = screen.getByRole("checkbox", { name: "Rita Gomes" });
    expect(rita).toBeChecked();
    expect(rita).toHaveAccessibleDescription("rita@example.com");
    expect(screen.getByText("41 emails")).toBeVisible();

    fireEvent.click(screen.getByRole("checkbox", { name: "Marta Ferreira" }));
    expect(onChange).toHaveBeenLastCalledWith(["rita", "marta"]);

    fireEvent.click(rita);
    expect(onChange).toHaveBeenLastCalledWith([]);
  });
});
