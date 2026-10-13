import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { Toast, UndoProvider, useUndoToast } from "./Toast";

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
