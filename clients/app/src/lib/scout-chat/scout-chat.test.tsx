import { fireEvent, render, renderHook, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import {
  conversationRef,
  ruleNoticeRef,
  ruleRef,
  ScoutChatHost,
  ScoutChatProvider,
  useScoutChat,
  type OpenScoutChat,
} from "./scout-chat";

function Opens() {
  const open = useScoutChat();
  return (
    <button
      type="button"
      onClick={(event) => {
        open?.(conversationRef("c-1"), { from: event.currentTarget });
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

  it("opens the app's panel about a record, from what was pressed", () => {
    const open = vi.fn();
    render(
      <ScoutChatProvider open={open}>
        <Opens />
      </ScoutChatProvider>,
    );
    const button = screen.getByRole("button", { name: "Ask Scout" });
    button.click();

    expect(open).toHaveBeenCalledWith(
      { type: "conversations.conversation", id: "c-1" },
      { from: button },
    );
  });

  it("names each record type a chat may be about", () => {
    expect(ruleRef("r-1")).toEqual({ type: "conversations.rule", id: "r-1" });
    expect(ruleNoticeRef("n-1")).toEqual({ type: "conversations.rule_notice", id: "n-1" });
  });

  it("tells the host above how to open the panel while one is mounted", () => {
    const open = vi.fn();
    const hosted: { current?: OpenScoutChat | undefined } = {};
    function Root() {
      const [mounted, setMounted] = useState(true);
      return (
        <ScoutChatHost
          onOpener={(next?: OpenScoutChat) => {
            hosted.current = next;
          }}
        >
          {mounted ? (
            <ScoutChatProvider open={open}>
              <button
                type="button"
                onClick={() => {
                  setMounted(false);
                }}
              >
                Leave
              </button>
            </ScoutChatProvider>
          ) : null}
        </ScoutChatHost>
      );
    }
    render(<Root />);

    hosted.current?.(null, { ask: "What needs me today?" });
    expect(open).toHaveBeenCalledWith(null, { ask: "What needs me today?" });
    fireEvent.click(screen.getByRole("button", { name: "Leave" }));
    expect(hosted.current).toBeUndefined();
  });
});
