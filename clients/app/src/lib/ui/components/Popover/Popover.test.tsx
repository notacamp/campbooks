import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";
import { AccountButton } from "../Shell/Shell";

import { Popover, PopoverSection } from "./Popover";

function renderPopover(onOpenChange = vi.fn()) {
  renderUi(
    <div>
      <p>Outside</p>
      <Popover
        label="Account"
        onOpenChange={onOpenChange}
        renderTrigger={(props) => (
          <AccountButton {...props} name="Ana Duarte" email="ana@example.com" />
        )}
      >
        {(close) => (
          <PopoverSection title="Theme">
            <button type="button" onClick={close}>
              Light
            </button>
          </PopoverSection>
        )}
      </Popover>
    </div>,
  );
  return onOpenChange;
}

describe("Popover", () => {
  it("opens from its trigger and focuses its first control", () => {
    const onOpenChange = renderPopover();
    const trigger = screen.getByRole("button", { name: /Ana Duarte/u });
    expect(trigger).toHaveAttribute("aria-expanded", "false");

    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("dialog", { name: "Account" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Light" })).toHaveFocus();
    expect(onOpenChange).toHaveBeenCalledWith(true);
  });

  it("closes on Escape and hands focus back to the trigger", () => {
    renderPopover();
    const trigger = screen.getByRole("button", { name: /Ana Duarte/u });
    fireEvent.click(trigger);

    fireEvent.keyDown(screen.getByRole("button", { name: "Light" }), { key: "Escape" });

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(trigger).toHaveFocus();
  });

  it("closes on a click outside, or from its content", () => {
    renderPopover();
    const trigger = screen.getByRole("button", { name: /Ana Duarte/u });
    fireEvent.click(trigger);
    fireEvent.pointerDown(screen.getByRole("button", { name: "Light" }));
    expect(screen.getByRole("dialog")).toBeVisible();
    fireEvent.keyDown(screen.getByRole("button", { name: "Light" }), { key: "a" });

    fireEvent.pointerDown(screen.getByText("Outside"));
    expect(screen.queryByRole("dialog")).toBeNull();

    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("button", { name: "Light" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
