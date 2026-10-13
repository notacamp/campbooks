import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_SHORTCUTS } from "./catalog";
import { ShortcutRegistry } from "./registry";
import {
  ShortcutProvider,
  useShortcut,
  useShortcutDefinition,
  useShortcutDefinitions,
  useShortcuts,
} from "./ShortcutProvider";

function Bound({ onDone, enabled = true }: { onDone: () => void; enabled?: boolean }) {
  useShortcut("item.done", onDone, { enabled });
  return null;
}

function BoundList() {
  const { definitions, isBound } = useShortcutDefinitions();
  return (
    <p>
      {definitions
        .filter((definition) => isBound(definition.id))
        .map((definition) => definition.id)
        .join(",")}
    </p>
  );
}

describe("ShortcutProvider", () => {
  it("dispatches document key presses to bound handlers", () => {
    const onDone = vi.fn();
    render(
      <ShortcutProvider platform="other">
        <Bound onDone={onDone} />
      </ShortcutProvider>,
    );

    fireEvent.keyDown(document.body, { key: "e" });

    expect(onDone).toHaveBeenCalledOnce();
  });

  it("binds only while enabled and unbinds on unmount", () => {
    const onDone = vi.fn();
    const registry = new ShortcutRegistry(DEFAULT_SHORTCUTS);
    const { rerender, unmount } = render(
      <ShortcutProvider registry={registry} platform="other">
        <Bound onDone={onDone} enabled={false} />
      </ShortcutProvider>,
    );
    expect(registry.isBound("item.done")).toBe(false);

    rerender(
      <ShortcutProvider registry={registry} platform="other">
        <Bound onDone={onDone} />
      </ShortcutProvider>,
    );
    expect(registry.isBound("item.done")).toBe(true);

    unmount();
    expect(registry.isBound("item.done")).toBe(false);
  });

  it("always calls the latest handler", () => {
    const calls: number[] = [];
    function Counter() {
      const [count, setCount] = useState(0);
      useShortcut("item.done", () => {
        calls.push(count);
      });
      return (
        <button
          type="button"
          onClick={() => {
            setCount(count + 1);
          }}
        >
          {count}
        </button>
      );
    }
    render(
      <ShortcutProvider platform="other">
        <Counter />
      </ShortcutProvider>,
    );

    fireEvent.click(screen.getByRole("button"));
    fireEvent.keyDown(document.body, { key: "e" });

    expect(calls).toEqual([1]);
  });

  it("re-renders readers when bindings change", () => {
    const registry = new ShortcutRegistry(DEFAULT_SHORTCUTS);
    render(
      <ShortcutProvider registry={registry} platform="other">
        <BoundList />
      </ShortcutProvider>,
    );
    expect(screen.getByRole("paragraph")).toHaveTextContent("");

    act(() => {
      registry.bind("undo", vi.fn());
    });

    expect(screen.getByRole("paragraph")).toHaveTextContent("undo");
  });

  it("falls back to the default registry outside a provider", () => {
    function Probe() {
      const { registry } = useShortcuts();
      const definition = useShortcutDefinition("nav.inbox");
      return <p>{`${definition?.keys ?? ""} ${String(registry.definitions().length)}`}</p>;
    }
    render(<Probe />);

    expect(screen.getByRole("paragraph")).toHaveTextContent(
      `g i ${String(DEFAULT_SHORTCUTS.length)}`,
    );
  });
});
