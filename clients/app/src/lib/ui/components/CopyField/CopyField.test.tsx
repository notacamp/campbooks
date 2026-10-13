import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_SHORTCUTS, ShortcutRegistry } from "~/lib/shortcuts";

import { renderUi } from "../../../../../test/render";

import { CopyField } from "./CopyField";

const props = {
  label: "Invitation link",
  value: "https://example.com/invitations/accept?token=t",
  copyLabel: "Copy link",
  copiedLabel: "Link copied.",
  failedLabel: "Couldn’t copy.",
};

describe("CopyField", () => {
  it("copies the value and says so", async () => {
    const copy = vi.fn(() => Promise.resolve());
    renderUi(<CopyField {...props} copy={copy} />);

    expect(screen.getByRole("textbox", { name: "Invitation link" })).toHaveValue(props.value);
    fireEvent.click(screen.getByRole("button", { name: "Copy link" }));

    expect(await screen.findByText("Link copied.")).toBeVisible();
    expect(copy).toHaveBeenCalledWith(props.value);
  });

  it("selects the value when the clipboard refuses", async () => {
    renderUi(<CopyField {...props} copy={() => Promise.reject(new Error("denied"))} />);

    fireEvent.click(screen.getByRole("button", { name: "Copy link" }));

    expect(await screen.findByText("Couldn’t copy.")).toBeVisible();
    expect(screen.getByRole("textbox")).toHaveFocus();
  });

  it("copies from its shortcut, shown on the button", async () => {
    const registry = new ShortcutRegistry([
      ...DEFAULT_SHORTCUTS,
      { id: "demo.copy", keys: "c", scope: "item", group: "item", label: "shortcuts:undo" },
    ]);
    const copy = vi.fn(() => Promise.resolve());
    renderUi(<CopyField {...props} copy={copy} shortcut="demo.copy" />, { registry });

    expect(screen.getByRole("button", { name: /Copy link/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "C",
    );
    fireEvent.keyDown(document.body, { key: "c" });
    fireEvent.focus(screen.getByRole("textbox"));

    await waitFor(() => {
      expect(copy).toHaveBeenCalledOnce();
    });
  });
});
