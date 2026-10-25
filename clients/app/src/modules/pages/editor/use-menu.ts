import type { SuggestionProps } from "@tiptap/suggestion";
import { useEffect, useRef, useState } from "react";

import type { MenuBridge } from "./menus";

/** An open `/` or `@` menu: what it lists, where, and how to pick. */
export interface OpenMenu<TItem> {
  items: TItem[];
  loading: boolean;
  anchor: { x: number; y: number };
  command: (item: TItem) => void;
}

/**
 * The state of one suggestion menu and the bridge the editor's plugin talks
 * to: the plugin tells the page when the menu opens, changes and closes, and
 * asks it what to do with the arrow keys, Enter and Escape. The items stay
 * the page's to draw.
 */
export function useMenu<TItem>(close: () => void) {
  const [menu, setMenu] = useState<OpenMenu<TItem> | undefined>();
  const [active, setActive] = useState(0);
  const latest = useRef({ menu, active, close });
  useEffect(() => {
    latest.current = { menu, active, close };
  });
  const [bridge] = useState<MenuBridge<TItem>>(() => ({
    show: (props: SuggestionProps<TItem, TItem>) => {
      const rect = props.clientRect?.();
      setMenu({
        items: props.items,
        loading: props.loading,
        anchor:
          rect === null || rect === undefined
            ? { x: 0, y: 0 }
            : { x: rect.left, y: rect.bottom + 4 },
        command: props.command,
      });
      setActive(0);
    },
    hide: () => {
      setMenu(undefined);
    },
    key: (event) => {
      const { menu: open, active: at, close: exit } = latest.current;
      if (open === undefined) {
        return false;
      }
      const count = open.items.length;
      switch (event.key) {
        case "ArrowDown": {
          setActive((at + 1) % Math.max(count, 1));
          return true;
        }
        case "ArrowUp": {
          setActive((at - 1 + Math.max(count, 1)) % Math.max(count, 1));
          return true;
        }
        case "Enter":
        case "Tab": {
          const item = open.items[at];
          if (item === undefined) {
            return false;
          }
          open.command(item);
          return true;
        }
        case "Escape": {
          exit();
          return true;
        }
        default: {
          return false;
        }
      }
    },
  }));
  return { bridge, menu, active, setActive };
}
