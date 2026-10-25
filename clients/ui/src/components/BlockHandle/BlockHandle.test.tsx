import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import { BlockHandle } from "./BlockHandle";

describe("BlockHandle", () => {
  it("adds below on the plus, opens the menu on the grip and drags from the grip", () => {
    const onAdd = vi.fn();
    const onMenu = vi.fn();
    const onDragStart = vi.fn();
    renderUi(
      <BlockHandle
        top={4}
        addLabel="Add a block below"
        onAdd={onAdd}
        menuLabel="Drag to move, click for the block menu"
        onMenu={onMenu}
        onDragStart={onDragStart}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Add a block below" }));
    expect(onAdd).toHaveBeenCalledTimes(1);
    const grip = screen.getByRole("button", { name: /Drag to move/u });
    fireEvent.click(grip);
    expect(onMenu).toHaveBeenCalledTimes(1);
    fireEvent.dragStart(grip);
    expect(onDragStart).toHaveBeenCalledTimes(1);
  });
});
