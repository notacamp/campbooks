import { render, renderHook, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ReplyOpenerProvider, useOpenReply } from "./reply-opener";

const REPLY = {
  conversationId: "c-1",
  stream: "primary",
  name: "Marta Ferreira",
  intent: "Confirm Thursday at 10",
};

function Opens() {
  const open = useOpenReply();
  return (
    <button
      type="button"
      onClick={() => {
        open?.(REPLY);
      }}
    >
      {open === undefined ? "nothing" : "Write the draft"}
    </button>
  );
}

describe("useOpenReply", () => {
  it("is nothing where no dialog is provided", () => {
    const { result } = renderHook(() => useOpenReply());

    expect(result.current).toBeUndefined();
  });

  it("opens the app's Reply dialog with the intent", () => {
    const open = vi.fn();
    render(
      <ReplyOpenerProvider open={open}>
        <Opens />
      </ReplyOpenerProvider>,
    );
    screen.getByRole("button", { name: "Write the draft" }).click();

    expect(open).toHaveBeenCalledWith(REPLY);
  });
});
