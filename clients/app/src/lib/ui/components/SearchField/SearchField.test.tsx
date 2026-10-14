import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { SearchField } from "./SearchField";

describe("SearchField", () => {
  it("reports typing and shows the shortcut that reaches it", () => {
    const onValueChange = vi.fn();
    renderUi(
      <SearchField label="Search conversations" value="" onValueChange={onValueChange} keys="/" />,
    );

    fireEvent.change(screen.getByRole("searchbox", { name: "Search conversations" }), {
      target: { value: "lease" },
    });

    expect(onValueChange).toHaveBeenCalledWith("lease");
    expect(screen.getByText("/")).toBeVisible();
  });

  it("lists the filters as removable chips and clears the text", () => {
    const onRemove = vi.fn();
    const onClear = vi.fn();
    renderUi(
      <SearchField
        label="Search"
        value="invoice from:ana"
        onValueChange={vi.fn()}
        onClear={onClear}
        chips={[{ id: "from", label: "From ana", onRemove }]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Remove filter: From ana" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));

    expect(onRemove).toHaveBeenCalledOnce();
    expect(onClear).toHaveBeenCalledOnce();
    expect(screen.getByRole("list", { name: "Search filters" })).toBeVisible();
  });

  it("offers no clear button for an empty field", () => {
    renderUi(<SearchField label="Search" value="" onValueChange={vi.fn()} onClear={vi.fn()} />);

    expect(screen.queryByRole("button", { name: "Clear search" })).toBeNull();
  });

  it("passes combobox attributes to the input", () => {
    renderUi(
      <SearchField
        label="Search"
        value=""
        onValueChange={vi.fn()}
        role="combobox"
        aria-expanded="true"
        aria-controls="results"
      />,
    );

    expect(screen.getByRole("combobox", { name: "Search" })).toHaveAttribute(
      "aria-controls",
      "results",
    );
  });
});
