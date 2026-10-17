import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Toast, UndoProvider, useUndoToast, type PendingHandle } from "./Toast";

function Harness() {
  const undo = useUndoToast();
  const [count, setCount] = useState(0);
  return (
    <div>
      <p>{`count ${String(count)}`}</p>
      <button
        type="button"
        onClick={() => {
          setCount((value) => value + 1);
          undo.push({
            label: "Done · Sofia",
            undo: () => {
              setCount((value) => value - 1);
            },
            redo: () => {
              setCount((value) => value + 1);
            },
          });
        }}
      >
        act
      </button>
      <ul>
        {undo.history.map((entry) => (
          <li key={entry.id}>
            <button
              type="button"
              onClick={() => {
                undo.toggle(entry.id);
              }}
            >
              {`${entry.undone ? "redo" : "undo"} ${String(entry.id)}`}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

describe("UndoProvider", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("takes a failed action out of the history, so Z skips it", () => {
    const undone: string[] = [];
    function Retracting() {
      const undo = useUndoToast();
      return (
        <div>
          <button
            type="button"
            onClick={() => {
              undo.push({
                label: "Done · Ana",
                undo: () => {
                  undone.push("ana");
                },
              });
              const failed = undo.push({
                label: "Done · Rui",
                undo: () => {
                  undone.push("rui");
                },
              });
              undo.retract(failed);
            }}
          >
            act
          </button>
          <p>{`entries ${String(undo.history.length)}`}</p>
        </div>
      );
    }
    renderUi(
      <UndoProvider>
        <Retracting />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "act" }));
    expect(screen.getByText("entries 1")).toBeVisible();
    fireEvent.keyDown(document, { key: "z" });
    expect(undone).toEqual(["ana"]);
  });

  it("shows each action with Undo in a polite live region", () => {
    renderUi(
      <UndoProvider>
        <Harness />
      </UndoProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "act" }));

    expect(screen.getByRole("status", { name: "Notifications" })).toHaveTextContent("Done · Sofia");
    fireEvent.click(screen.getByRole("button", { name: /^Undo/ }));
    expect(screen.getByRole("paragraph")).toHaveTextContent("count 0");
    expect(screen.getByRole("status")).toHaveTextContent("Undone: Done · Sofia");
  });

  it("undoes the latest action with Z, stack-wise", () => {
    renderUi(
      <UndoProvider>
        <Harness />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "act" }));
    fireEvent.click(screen.getByRole("button", { name: "act" }));

    fireEvent.keyDown(document.body, { key: "z" });
    fireEvent.keyDown(document.body, { key: "z" });
    fireEvent.keyDown(document.body, { key: "z" });

    expect(screen.getByRole("paragraph")).toHaveTextContent("count 0");
  });

  it("redoes an undone entry from the history", () => {
    renderUi(
      <UndoProvider>
        <Harness />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "act" }));
    fireEvent.click(screen.getByRole("button", { name: "undo 1" }));
    fireEvent.click(screen.getByRole("button", { name: "redo 1" }));

    expect(screen.getByRole("paragraph")).toHaveTextContent("count 1");
    expect(screen.getByRole("status")).toHaveTextContent("Redone: Done · Sofia");
  });

  it("hides the toast after a while but keeps the history", () => {
    vi.useFakeTimers();
    renderUi(
      <UndoProvider toastMs={1000}>
        <Harness />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "act" }));

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    expect(screen.getByRole("button", { name: "undo 1" })).toBeInTheDocument();
  });

  it("keeps an action's toast for its own window, and says things without Undo", () => {
    vi.useFakeTimers();
    function Window() {
      const undo = useUndoToast();
      return (
        <>
          <button
            type="button"
            onClick={() => {
              undo.push({ label: "Sending to Ana", undo: vi.fn(), durationMs: 10_000 });
            }}
          >
            send
          </button>
          <button
            type="button"
            onClick={() => {
              undo.notify("Too late: it was already sent");
            }}
          >
            late
          </button>
        </>
      );
    }
    renderUi(
      <UndoProvider toastMs={1000}>
        <Window />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "send" }));
    act(() => {
      vi.advanceTimersByTime(9000);
    });
    expect(screen.getByRole("status")).toHaveTextContent("Sending to Ana");
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    fireEvent.click(screen.getByRole("button", { name: "late" }));
    expect(screen.getByRole("status")).toHaveTextContent("Too late: it was already sent");
    expect(screen.queryByRole("button", { name: /^Undo/ })).not.toBeInTheDocument();
  });

  it("shows a pending action, then how it went, with Undo once done", () => {
    vi.useFakeTimers();
    let handle: PendingHandle | undefined;
    const undone = vi.fn();
    function Pending() {
      const undo = useUndoToast();
      return (
        <button
          type="button"
          onClick={() => {
            handle = undo.pending({ label: "Sending to Marta…" });
          }}
        >
          send
        </button>
      );
    }
    renderUi(
      <UndoProvider toastMs={1000}>
        <Pending />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "send" }));

    expect(screen.getByRole("status")).toHaveTextContent("Sending to Marta…");
    expect(screen.getByRole("status").querySelector("[data-state='thinking']")).not.toBeNull();
    // No Undo: this pending action can't be taken back.
    expect(screen.queryByRole("button", { name: /^Undo/ })).not.toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.getByRole("status")).toHaveTextContent("Sending to Marta…");

    act(() => {
      handle?.done({ label: "Sent to Marta", undo: undone });
    });
    expect(screen.getByRole("status")).toHaveTextContent("Sent to Marta");
    fireEvent.keyDown(document.body, { key: "z" });
    expect(undone).toHaveBeenCalledOnce();
  });

  it("cancels a pending action with Z while it allows it", () => {
    const cancel = vi.fn();
    let handle: PendingHandle | undefined;
    function Pending() {
      const undo = useUndoToast();
      return (
        <button
          type="button"
          onClick={() => {
            handle = undo.pending({
              label: "Sending to Marta…",
              cancel,
              cancelled: "Not sent. Your draft is back.",
            });
          }}
        >
          send
        </button>
      );
    }
    renderUi(
      <UndoProvider>
        <Pending />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "send" }));
    expect(screen.getByRole("button", { name: /^Undo/ })).toBeInTheDocument();

    fireEvent.keyDown(document.body, { key: "z" });

    expect(cancel).toHaveBeenCalledOnce();
    expect(screen.getByRole("status")).toHaveTextContent("Not sent. Your draft is back.");
    // Settled: a late outcome changes nothing.
    act(() => {
      handle?.done({ label: "Sent to Marta" });
    });
    expect(screen.getByRole("status")).toHaveTextContent("Not sent. Your draft is back.");
  });

  it("keeps a failure up until Retry or dismiss", () => {
    vi.useFakeTimers();
    const retry = vi.fn();
    function Failing() {
      const undo = useUndoToast();
      return (
        <>
          <button
            type="button"
            onClick={() => {
              undo.pending({ label: "Sending to Marta…" }).fail({
                label: "Couldn't send to Marta. Your draft is kept.",
                retry,
              });
            }}
          >
            send
          </button>
          <button
            type="button"
            onClick={() => {
              undo.fail({ label: "Couldn't snooze Pedro: nothing changed." });
            }}
          >
            snooze
          </button>
        </>
      );
    }
    renderUi(
      <UndoProvider toastMs={1000}>
        <Failing />
      </UndoProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "send" }));
    act(() => {
      vi.advanceTimersByTime(60_000);
    });

    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't send to Marta.");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retry).toHaveBeenCalledOnce();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "snooze" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't snooze Pedro");
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("refuses useUndoToast outside a provider", () => {
    vi.spyOn(console, "error").mockImplementation(() => {
      // React reports the thrown render error; the assertion is what matters.
    });
    expect(() => render(<Harness />)).toThrow(/inside <UndoProvider>/);
  });
});

describe("Toast", () => {
  it("announces failures as alerts", () => {
    renderUi(<Toast tone="alert">Not sent to Sofia.</Toast>);

    expect(screen.getByRole("alert")).toHaveTextContent("Not sent to Sofia.");
  });
});
