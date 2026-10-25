import type { ReactNode } from "react";

import { Icon, type IconName } from "@notacamp/ui";

import styles from "./FormatBar.module.css";

export interface FormatBarItem {
  id: string;
  /** Accessible name ("Insert a block", "Bold"). */
  label: string;
  /** A line icon, or a short text glyph ("/", "@", "B"). */
  icon?: IconName | undefined;
  glyph?: string | undefined;
  /** The mark or block is on at the caret. */
  pressed?: boolean | undefined;
  onPress: () => void;
}

export interface FormatBarProps {
  /** Accessible name of the bar ("Format"). */
  label: string;
  items: readonly FormatBarItem[];
  /** The last button, on the far end (hide the keyboard). */
  end?: ReactNode | undefined;
  /** Shown from the desktop width up too (otherwise on phones only). */
  always?: boolean | undefined;
}

/**
 * The bar above the phone keyboard: insert a block, mention, to-do, bold,
 * italic. Its buttons keep the editor's selection: they do not take focus on
 * press.
 */
export function FormatBar({ label, items, end, always = false }: FormatBarProps) {
  return (
    <div
      className={styles["bar"]}
      role="toolbar"
      aria-label={label}
      data-always={always || undefined}
    >
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          className={styles["button"]}
          aria-label={item.label}
          aria-pressed={item.pressed}
          onMouseDown={(event) => {
            event.preventDefault();
          }}
          onClick={item.onPress}
        >
          {item.icon === undefined ? item.glyph : <Icon name={item.icon} />}
        </button>
      ))}
      <span className={styles["spacer"]} />
      {end}
    </div>
  );
}
