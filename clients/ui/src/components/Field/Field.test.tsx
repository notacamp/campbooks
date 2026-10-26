import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { Select, TextField } from "./Field";

describe("TextField", () => {
  it("labels the input and describes it with its hint and error", () => {
    renderUi(<TextField label="Email" hint="Their work address." error="Not an email." />);

    const input = screen.getByRole("textbox", { name: "Email" });
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Their work address. Not an email.");
  });

  it("is valid and undescribed without hint or error", () => {
    renderUi(<TextField label="Name" hideLabel size="lg" />);

    const input = screen.getByRole("textbox", { name: "Name" });
    expect(input).not.toHaveAttribute("aria-invalid");
    expect(input).not.toHaveAttribute("aria-describedby");
    expect(input).toHaveAttribute("data-size", "lg");
  });
});

describe("Select", () => {
  it("reports the chosen option", () => {
    const onChange = vi.fn();
    renderUi(
      <Select
        label="Role"
        value="member"
        onChange={onChange}
        options={[
          { value: "member", label: "Member" },
          { value: "admin", label: "Admin" },
        ]}
      />,
    );

    fireEvent.change(screen.getByRole("combobox", { name: "Role" }), {
      target: { value: "admin" },
    });

    expect(onChange).toHaveBeenCalledWith("admin");
  });
});
