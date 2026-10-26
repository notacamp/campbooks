import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { TitleField } from "./TitleField";

describe("TitleField", () => {
  it("is a text box named for what it holds, and reports what is typed", () => {
    const onChange = vi.fn();
    renderUi(<TitleField label="Page title" value="Weekly plan" onChange={onChange} />);
    const field = screen.getByRole("textbox", { name: "Page title" });
    expect(field).toHaveValue("Weekly plan");
    fireEvent.change(field, { target: { value: "Weekly plan v2" } });
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("cannot be changed when read-only", () => {
    renderUi(<TitleField label="Page title" value="Handbook" readOnly onChange={vi.fn()} />);
    expect(screen.getByRole("textbox", { name: "Page title" })).toHaveAttribute("readonly");
  });

  it("takes no line breaks: Enter does nothing and a pasted break becomes a space", () => {
    const onChange = vi.fn();
    renderUi(<TitleField label="Page title" defaultValue="" onChange={onChange} />);
    const field = screen.getByRole("textbox", { name: "Page title" });
    const enter = fireEvent.keyDown(field, { key: "Enter" });
    expect(enter).toBe(false);
    fireEvent.change(field, { target: { value: "Weekly\nplan" } });
    expect(field).toHaveValue("Weekly plan");
    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
