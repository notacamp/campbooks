import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";
import { InlineConfirm } from "../InlineConfirm/InlineConfirm";

import { List, ListRow } from "./List";

describe("List and ListRow", () => {
  it("renders named rows with their parts", () => {
    renderUi(
      <List label="Members">
        <ListRow
          title="Ana"
          badge="You"
          meta="ana@example.com"
          trailing={<button type="button">Remove</button>}
        >
          <p>Below</p>
        </ListRow>
        <ListRow muted title="rui@example.com" />
      </List>,
    );

    const list = screen.getByRole("list", { name: "Members" });
    expect(list.children).toHaveLength(2);
    expect(screen.getByText("You")).toBeInTheDocument();
    expect(screen.getByText("Below")).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")[1]).toHaveAttribute("data-muted", "true");
  });
});

describe("InlineConfirm", () => {
  it("focuses Cancel, confirms, and cancels on Escape", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    renderUi(
      <InlineConfirm
        confirmLabel="Remove"
        cancelLabel="Cancel"
        onConfirm={onConfirm}
        onCancel={onCancel}
      >
        Remove Ana?
      </InlineConfirm>,
    );

    expect(screen.getByRole("group", { name: "Remove Ana?" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();

    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onConfirm).toHaveBeenCalledOnce();

    fireEvent.keyDown(screen.getByRole("button", { name: "Cancel" }), { key: "Escape" });
    fireEvent.keyDown(screen.getByRole("button", { name: "Cancel" }), { key: "Enter" });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("disables both buttons while busy", () => {
    renderUi(
      <InlineConfirm
        confirmLabel="Remove"
        cancelLabel="Cancel"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
        busy
      >
        Removing
      </InlineConfirm>,
    );

    expect(screen.getByRole("button", { name: "Remove" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  });
});
