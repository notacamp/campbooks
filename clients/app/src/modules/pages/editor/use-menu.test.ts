import type { SuggestionProps } from "@tiptap/suggestion";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useMenu } from "./use-menu";

const key = (name: string) => new KeyboardEvent("keydown", { key: name });

/** The little of a plugin's props the menu reads. */
function props(items: string[], command = vi.fn()): SuggestionProps<string, string> {
  return {
    items,
    loading: false,
    command,
    clientRect: () => new DOMRect(10, 20, 5, 12),
  } as unknown as SuggestionProps<string, string>;
}

describe("a suggestion menu", () => {
  it("opens under the caret and closes", () => {
    const { result } = renderHook(() => useMenu<string>(vi.fn()));
    expect(result.current.menu).toBeUndefined();
    act(() => {
      result.current.bridge.show(props(["a", "b"]));
    });
    expect(result.current.menu?.items).toEqual(["a", "b"]);
    expect(result.current.menu?.anchor).toEqual({ x: 10, y: 36 });
    act(() => {
      result.current.bridge.hide();
    });
    expect(result.current.menu).toBeUndefined();
  });

  it("is at the top left when the editor cannot say where the caret is", () => {
    const { result } = renderHook(() => useMenu<string>(vi.fn()));
    act(() => {
      result.current.bridge.show({ ...props(["a"]), clientRect: null });
    });
    expect(result.current.menu?.anchor).toEqual({ x: 0, y: 0 });
  });

  it("moves with the arrows, picks with Enter and leaves with Escape", () => {
    const close = vi.fn();
    const command = vi.fn();
    const { result } = renderHook(() => useMenu<string>(close));
    expect(result.current.bridge.key(key("ArrowDown"))).toBe(false);
    act(() => {
      result.current.bridge.show(props(["a", "b", "c"], command));
    });
    act(() => {
      expect(result.current.bridge.key(key("ArrowDown"))).toBe(true);
    });
    expect(result.current.active).toBe(1);
    act(() => {
      expect(result.current.bridge.key(key("ArrowUp"))).toBe(true);
    });
    expect(result.current.active).toBe(0);
    act(() => {
      expect(result.current.bridge.key(key("ArrowUp"))).toBe(true);
    });
    expect(result.current.active).toBe(2);
    act(() => {
      expect(result.current.bridge.key(key("Enter"))).toBe(true);
    });
    expect(command).toHaveBeenCalledWith("c");
    act(() => {
      expect(result.current.bridge.key(key("Escape"))).toBe(true);
    });
    expect(close).toHaveBeenCalledTimes(1);
    expect(result.current.bridge.key(key("x"))).toBe(false);
  });

  it("leaves Enter to the editor when nothing matches", () => {
    const { result } = renderHook(() => useMenu<string>(vi.fn()));
    act(() => {
      result.current.bridge.show(props([]));
    });
    expect(result.current.bridge.key(key("Enter"))).toBe(false);
  });
});
