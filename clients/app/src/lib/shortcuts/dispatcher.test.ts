import { describe, expect, it, vi } from "vitest";

import { createDispatcher, isTypingTarget, SEQUENCE_TIMEOUT_MS } from "./dispatcher";
import { ShortcutRegistry, type ShortcutDefinition } from "./registry";

const definitions: ShortcutDefinition[] = [
  { id: "done", keys: "e", scope: "item", group: "item", label: "shortcuts:item.done" },
  { id: "inbox", keys: "g i", scope: "global", group: "navigation", label: "shortcuts:nav.inbox" },
  { id: "home", keys: "g h", scope: "global", group: "navigation", label: "shortcuts:nav.home" },
  {
    id: "menu",
    keys: "mod+k",
    scope: "global",
    group: "general",
    label: "shortcuts:command.open",
    allowInInputs: true,
  },
  { id: "sheet", keys: "?", scope: "global", group: "general", label: "shortcuts:sheet.open" },
];

function setup(platform: "apple" | "other" = "other") {
  let clock = 0;
  const registry = new ShortcutRegistry(definitions);
  const handlers = Object.fromEntries(
    definitions.map((definition) => {
      const handler = vi.fn();
      registry.bind(definition.id, handler);
      return [definition.id, handler];
    }),
  );
  const dispatch = createDispatcher(registry, platform, { now: () => clock });
  const press = (init: KeyboardEventInit, target: EventTarget = document.body) => {
    const event = new KeyboardEvent("keydown", { ...init, cancelable: true });
    Object.defineProperty(event, "target", { value: target });
    dispatch(event);
    return event;
  };
  const advance = (ms: number) => {
    clock += ms;
  };
  return { registry, handlers, press, advance };
}

describe("createDispatcher", () => {
  it("runs a single-key shortcut and prevents the default action", () => {
    const { handlers, press } = setup();

    const event = press({ key: "e" });

    expect(handlers["done"]).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it("runs a sequence typed within the timeout", () => {
    const { handlers, press, advance } = setup();

    press({ key: "g" });
    advance(SEQUENCE_TIMEOUT_MS - 1);
    press({ key: "i" });

    expect(handlers["inbox"]).toHaveBeenCalledOnce();
    expect(handlers["home"]).not.toHaveBeenCalled();
  });

  it("forgets a sequence after the timeout", () => {
    const { handlers, press, advance } = setup();

    press({ key: "g" });
    advance(SEQUENCE_TIMEOUT_MS + 1);
    press({ key: "i" });

    expect(handlers["inbox"]).not.toHaveBeenCalled();
  });

  it("treats a key that breaks a sequence as a fresh press", () => {
    const { handlers, press } = setup();

    press({ key: "g" });
    press({ key: "e" });

    expect(handlers["done"]).toHaveBeenCalledOnce();
  });

  it("uses ⌘ on Apple platforms and Ctrl elsewhere for mod", () => {
    const apple = setup("apple");
    apple.press({ key: "k", ctrlKey: true });
    apple.press({ key: "k", metaKey: true });

    const other = setup("other");
    other.press({ key: "k", ctrlKey: true });

    expect(apple.handlers["menu"]).toHaveBeenCalledOnce();
    expect(other.handlers["menu"]).toHaveBeenCalledOnce();
  });

  it("matches a symbol typed with Shift", () => {
    const { handlers, press } = setup();

    press({ key: "?", shiftKey: true });

    expect(handlers["sheet"]).toHaveBeenCalledOnce();
  });

  it("ignores keys typed into fields unless the shortcut allows it", () => {
    const { handlers, press } = setup();
    const input = document.createElement("input");
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");

    press({ key: "e" }, input);
    press({ key: "e" }, editable);
    press({ key: "k", ctrlKey: true }, input);

    expect(handlers["done"]).not.toHaveBeenCalled();
    expect(handlers["menu"]).toHaveBeenCalledOnce();
  });

  it("ignores handled events, lone modifiers and unbound shortcuts", () => {
    const { registry, handlers, press } = setup();
    const handled = new KeyboardEvent("keydown", { key: "e", cancelable: true });
    handled.preventDefault();
    createDispatcher(registry, "other")(handled);
    press({ key: "Shift" });
    press({ key: "x" });

    expect(handlers["done"]).not.toHaveBeenCalled();
  });
});

describe("isTypingTarget", () => {
  it("knows text fields from buttons and checkboxes", () => {
    const text = document.createElement("input");
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";

    expect(isTypingTarget(text)).toBe(true);
    expect(isTypingTarget(checkbox)).toBe(false);
    expect(isTypingTarget(document.createElement("textarea"))).toBe(true);
    expect(isTypingTarget(document.createElement("select"))).toBe(true);
    expect(isTypingTarget(document.createElement("button"))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});
