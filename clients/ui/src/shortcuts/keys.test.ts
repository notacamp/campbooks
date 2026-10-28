import { describe, expect, it } from "vitest";

import {
  ariaKeyShortcut,
  chordFromEvent,
  detectPlatform,
  formatBinding,
  normalizeBinding,
  parseBinding,
} from "./keys";

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("parseBinding", () => {
  it("parses a single key, modifiers and a sequence", () => {
    expect(parseBinding("e")).toEqual([{ key: "e", mod: false, shift: false, alt: false }]);
    expect(parseBinding("mod+shift+w")).toEqual([{ key: "w", mod: true, shift: true, alt: false }]);
    expect(parseBinding("g i").map((chord) => chord.key)).toEqual(["g", "i"]);
  });

  it("rejects a binding without a key or with an unknown modifier", () => {
    expect(() => parseBinding("mod+")).toThrow(/needs a key/);
    expect(() => parseBinding("shift")).toThrow(/needs a key/);
    expect(() => parseBinding("hyper+k")).toThrow(/unknown modifier "hyper"/);
  });

  it("normalises modifier order so equal bindings compare equal", () => {
    expect(normalizeBinding("shift+mod+W")).toBe(normalizeBinding("mod+shift+w"));
  });
});

describe("chordFromEvent", () => {
  it("reads a digit with a modifier and Shift as the digit, not what Shift types", () => {
    const chord = chordFromEvent(
      key({ key: "*", code: "Digit8", metaKey: true, shiftKey: true }),
      "apple",
    );
    expect(chord).toEqual({ key: "8", mod: true, shift: true, alt: false });
    // Without a modifier, "*" is still the symbol.
    expect(chordFromEvent(key({ key: "*", code: "Digit8", shiftKey: true }), "apple")?.key).toBe(
      "*",
    );
  });

  it("maps ⌘ to mod on Apple platforms and Ctrl elsewhere", () => {
    expect(chordFromEvent(key({ key: "k", metaKey: true }), "apple")?.mod).toBe(true);
    expect(chordFromEvent(key({ key: "k", metaKey: true }), "other")?.mod).toBe(false);
    expect(chordFromEvent(key({ key: "k", ctrlKey: true }), "other")?.mod).toBe(true);
  });

  it("ignores Shift for symbols that already encode it, keeps it for letters", () => {
    expect(chordFromEvent(key({ key: "?", shiftKey: true }), "other")?.shift).toBe(false);
    expect(chordFromEvent(key({ key: "W", shiftKey: true }), "other")).toEqual({
      key: "w",
      mod: false,
      shift: true,
      alt: false,
    });
  });

  it("returns null for a lone modifier key", () => {
    expect(chordFromEvent(key({ key: "Shift" }), "other")).toBeNull();
    expect(chordFromEvent(key({ key: "Meta" }), "apple")).toBeNull();
  });
});

describe("formatBinding", () => {
  it("draws platform-aware keycaps", () => {
    expect(formatBinding("mod+k", "apple")).toEqual([["⌘", "K"]]);
    expect(formatBinding("mod+k", "other")).toEqual([["Ctrl", "K"]]);
    expect(formatBinding("mod+shift+w", "apple")).toEqual([["⌘", "⇧", "W"]]);
    expect(formatBinding("alt+x", "other")).toEqual([["Alt", "X"]]);
  });

  it("names special keys and splits sequences", () => {
    expect(formatBinding("enter", "apple")).toEqual([["↵"]]);
    expect(formatBinding("escape", "other")).toEqual([["Esc"]]);
    expect(formatBinding("g i", "other")).toEqual([["G"], ["I"]]);
  });
});

describe("ariaKeyShortcut", () => {
  it("writes single chords in aria-keyshortcuts syntax", () => {
    expect(ariaKeyShortcut("e", "other")).toBe("E");
    expect(ariaKeyShortcut("mod+k", "apple")).toBe("Meta+K");
    expect(ariaKeyShortcut("mod+shift+w", "other")).toBe("Control+Shift+W");
    expect(ariaKeyShortcut("alt+enter", "other")).toBe("Alt+Enter");
    expect(ariaKeyShortcut("arrowup", "other")).toBe("ArrowUp");
  });

  it("leaves sequences out: the attribute cannot express them", () => {
    expect(ariaKeyShortcut("g i", "apple")).toBeUndefined();
  });
});

describe("detectPlatform", () => {
  it("recognises Apple platforms", () => {
    expect(detectPlatform({ platform: "MacIntel", userAgent: "" })).toBe("apple");
    expect(
      detectPlatform({ platform: "", userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0)" }),
    ).toBe("apple");
    expect(detectPlatform({ platform: "Win32", userAgent: "Windows NT 10.0" })).toBe("other");
  });
});
