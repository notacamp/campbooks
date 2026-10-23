import { render, renderHook, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ScoutChatProvider, useScoutChat } from "./scout-chat";

function Opens() {
  const open = useScoutChat();
  return (
    <button
      type="button"
      onClick={(event) => {
        open?.("c-1", event.currentTarget);
      }}
    >
      {open === undefined ? "nothing" : "Ask Scout"}
    </button>
  );
}

describe("useScoutChat", () => {
  it("is nothing where no panel is mounted", () => {
    const { result } = renderHook(() => useScoutChat());

    expect(result.current).toBeUndefined();
  });

  it("opens the app's panel for a conversation, from what was pressed", () => {
    const open = vi.fn();
    render(
      <ScoutChatProvider open={open}>
        <Opens />
      </ScoutChatProvider>,
    );
    const button = screen.getByRole("button", { name: "Ask Scout" });
    button.click();

    expect(open).toHaveBeenCalledWith("c-1", button);
  });
});
