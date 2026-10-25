import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "../../../../test/render";

import { IconPicker } from "./IconPicker";

describe("the icon picker", () => {
  it("shows only the icon, named for what it is", () => {
    renderWithProviders(<IconPicker value="star" onChange={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Page icon: Star" })).toBeTruthy();
  });

  it("starts from the document icon when the page has none", () => {
    renderWithProviders(<IconPicker value={null} onChange={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Page icon: Document" })).toBeTruthy();
  });

  it("offers every icon and reports the one picked", () => {
    const onChange = vi.fn();
    renderWithProviders(<IconPicker value="star" onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Page icon: Star" }));
    expect(screen.getByRole("button", { name: "Star" }).getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Mail" }));
    expect(onChange).toHaveBeenCalledWith("mail");
  });
});
