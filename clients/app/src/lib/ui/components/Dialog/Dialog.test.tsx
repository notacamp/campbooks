import { act, fireEvent, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useShortcut } from "~/lib/shortcuts";

import { renderUi } from "../../../../../test/render";

import { Dialog, DialogChoices, type DialogResult } from "./Dialog";

const TIMES = [
  { id: "today", label: "Later today", detail: "18:00" },
  { id: "tomorrow", label: "Tomorrow", detail: "08:00" },
  { id: "monday", label: "Monday", detail: "08:00" },
];

function Snooze({
  onChoose = vi.fn(),
  onClose = vi.fn(),
  onPageDone = vi.fn(),
}: {
  onChoose?: (id: string) => void;
  onClose?: () => void;
  onPageDone?: () => void;
}) {
  const [open, setOpen] = useState(true);
  // A page shortcut that must not fire behind the dialog.
  useShortcut("item.done", onPageDone);
  return (
    <>
      <button type="button">Snooze…</button>
      <Dialog
        open={open}
        onClose={() => {
          setOpen(false);
          onClose();
        }}
        title="Snooze Marta Ferreira until…"
        description="It leaves Home until then."
      >
        <DialogChoices label="When" choices={TIMES} onChoose={onChoose} />
      </Dialog>
    </>
  );
}

describe("Dialog", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("is a labelled modal that focuses its first choice", () => {
    renderUi(<Snooze />);
    const dialog = screen.getByRole("dialog", { name: "Snooze Marta Ferreira until…" });

    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toHaveAccessibleDescription("It leaves Home until then.");
    expect(screen.getByRole("button", { name: /Later today/ })).toHaveFocus();
    expect(screen.getByRole("button", { name: /Tomorrow/ })).toHaveAttribute(
      "aria-keyshortcuts",
      "2",
    );
  });

  it("picks a choice with its number key", () => {
    const onChoose = vi.fn();
    renderUi(<Snooze onChoose={onChoose} />);

    fireEvent.keyDown(screen.getByRole("button", { name: /Later today/ }), { key: "3" });

    expect(onChoose).toHaveBeenCalledWith("monday");
  });

  it("moves between choices with the arrows, wrapping", () => {
    renderUi(<Snooze />);
    const first = screen.getByRole("button", { name: /Later today/ });

    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(screen.getByRole("button", { name: /Tomorrow/ })).toHaveFocus();
    fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowUp" });
    fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowUp" });
    expect(screen.getByRole("button", { name: /Monday/ })).toHaveFocus();
  });

  it("closes on Escape with nothing chosen, and gives the focus back", () => {
    const onChoose = vi.fn();
    const onClose = vi.fn();
    renderUi(<Snooze onChoose={onChoose} onClose={onClose} />);

    fireEvent.keyDown(screen.getByRole("button", { name: /Later today/ }), { key: "Escape" });

    expect(onClose).toHaveBeenCalledOnce();
    expect(onChoose).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes on a click on the scrim, not on a click inside", () => {
    const onClose = vi.fn();
    renderUi(<Snooze onClose={onClose} />);
    const dialog = screen.getByRole("dialog");

    fireEvent.mouseDown(dialog);
    expect(onClose).not.toHaveBeenCalled();

    const scrim = dialog.parentElement;
    if (scrim === null) {
      throw new Error("no scrim");
    }
    fireEvent.mouseDown(scrim);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("keeps page shortcuts from firing behind it", () => {
    const onPageDone = vi.fn();
    renderUi(<Snooze onPageDone={onPageDone} />);

    fireEvent.keyDown(screen.getByRole("button", { name: /Later today/ }), { key: "e" });

    expect(onPageDone).not.toHaveBeenCalled();
  });

  it("shows a result that closes by itself after about a second", () => {
    vi.useFakeTimers();
    const onResultEnd = vi.fn();
    const result: DialogResult = { title: "Sent to Marta", detail: "Undo works for 10 seconds." };
    renderUi(
      <Dialog open onClose={vi.fn()} title="Reply" result={result} onResultEnd={onResultEnd} />,
    );

    expect(screen.getByRole("status")).toHaveTextContent("Sent to Marta");
    expect(screen.getByRole("dialog", { name: "Sent to Marta" })).toHaveFocus();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(onResultEnd).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(onResultEnd).toHaveBeenCalledOnce();
  });

  it("closes a result sooner on any key", () => {
    const onResultEnd = vi.fn();
    renderUi(
      <Dialog
        open
        onClose={vi.fn()}
        title="Reply"
        result={{ title: "Sent to Marta" }}
        onResultEnd={onResultEnd}
      />,
    );

    fireEvent.keyDown(screen.getByRole("dialog"), { key: "x" });

    expect(onResultEnd).toHaveBeenCalledOnce();
  });

  it("renders a footer and no visor when asked", () => {
    renderUi(
      <Dialog
        open
        onClose={vi.fn()}
        title="A rule for EDP invoices"
        visor={null}
        footer={<button type="button">Save smart rule</button>}
      >
        <p>When an invoice from EDP arrives, file it.</p>
      </Dialog>,
    );

    expect(screen.getByRole("button", { name: "Save smart rule" })).toBeInTheDocument();
    expect(screen.getByRole("dialog").querySelector("[data-state]")).toBeNull();
  });

  it("gives the focus to choices that arrive after it opened, when asked", () => {
    function Late() {
      const [ready, setReady] = useState(false);
      return (
        <Dialog open onClose={vi.fn()} title="Reply to Marta">
          <button
            type="button"
            onClick={() => {
              setReady(true);
            }}
          >
            Load
          </button>
          {ready ? (
            <DialogChoices label="Replies" choices={TIMES} onChoose={vi.fn()} focusOnShow />
          ) : null}
        </Dialog>
      );
    }
    renderUi(<Late />);

    fireEvent.click(screen.getByRole("button", { name: "Load" }));

    expect(screen.getByRole("button", { name: /Later today/ })).toHaveFocus();
  });

  it("takes the focus back when it left for nowhere: Esc still closes, page shortcuts don't fire", async () => {
    const onClose = vi.fn();
    const onPageDone = vi.fn();
    renderUi(<Snooze onClose={onClose} onPageDone={onPageDone} />);
    (document.activeElement as HTMLElement | null)?.blur();
    await act(async () => {
      await Promise.resolve();
    });
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveFocus();

    fireEvent.keyDown(dialog, { key: "e" });
    expect(onPageDone).not.toHaveBeenCalled();
    fireEvent.keyDown(dialog, { key: "2" });
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("takes the focus back when the focused control is replaced", async () => {
    const onChoose = vi.fn();
    function Steps() {
      const [step, setStep] = useState(1);
      return (
        <Dialog open onClose={vi.fn()} title="Two steps">
          {step === 1 ? (
            <button
              type="button"
              onClick={() => {
                setStep(2);
              }}
            >
              Next
            </button>
          ) : (
            <DialogChoices label="When" choices={TIMES} onChoose={onChoose} />
          )}
        </Dialog>
      );
    }
    renderUi(<Steps />);
    const next = screen.getByRole("button", { name: "Next" });
    next.focus();
    fireEvent.click(next);
    await act(async () => {
      await Promise.resolve();
    });
    const dialog = screen.getByRole("dialog");
    expect(document.activeElement === document.body).toBe(false);
    fireEvent.keyDown(document.activeElement ?? dialog, { key: "2" });
    expect(onChoose).toHaveBeenCalledWith("tomorrow");
  });

  it("listens on itself only: a key the page receives is the page's", () => {
    const onPageDone = vi.fn();
    renderUi(<Snooze onPageDone={onPageDone} />);
    // Nothing of the dialog's lost the focus: this key is the page's own.
    fireEvent.keyDown(document.body, { key: "e" });
    expect(onPageDone).toHaveBeenCalledOnce();
  });

  it("leaves the focus with someone typing when choices arrive", () => {
    function Typing() {
      const [ready, setReady] = useState(false);
      return (
        <Dialog open onClose={vi.fn()} title="Reply to Marta">
          <input
            aria-label="Own words"
            onChange={() => {
              setReady(true);
            }}
          />
          {ready ? (
            <DialogChoices label="Replies" choices={TIMES} onChoose={vi.fn()} focusOnShow />
          ) : null}
        </Dialog>
      );
    }
    renderUi(<Typing />);
    const field = screen.getByRole("textbox", { name: "Own words" });
    field.focus();
    fireEvent.change(field, { target: { value: "Say 2" } });

    expect(field).toHaveFocus();
  });
});
