import { useRef, type KeyboardEvent, type ReactNode } from "react";

import { cx, Icon, type IconName } from "@notacamp/ui";

import styles from "./Menu.module.css";

export interface MenuListProps {
  /** Names the menu. */
  label: string;
  children: ReactNode;
}

const ITEMS = '[role^="menuitem"]:not([disabled])';

/** A list of actions (a column's menu): arrows, Home and End move between them. */
export function MenuList({ label, children }: MenuListProps) {
  const list = useRef<HTMLDivElement>(null);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const items = [...(list.current?.querySelectorAll<HTMLElement>(ITEMS) ?? [])];
    if (items.length === 0) {
      return;
    }
    const at = items.indexOf(document.activeElement as HTMLElement);
    const move = (to: number) => {
      event.preventDefault();
      event.stopPropagation();
      items.at(to % items.length)?.focus();
    };
    switch (event.key) {
      case "ArrowDown": {
        move(at + 1);
        break;
      }
      case "ArrowUp": {
        move(at <= 0 ? -1 : at - 1);
        break;
      }
      case "Home": {
        move(0);
        break;
      }
      case "End": {
        move(-1);
        break;
      }
      default:
    }
  };
  return (
    <div
      ref={list}
      role="menu"
      tabIndex={-1}
      aria-label={label}
      className={styles["list"]}
      onKeyDown={onKeyDown}
    >
      {children}
    </div>
  );
}

export interface MenuItemProps {
  children: ReactNode;
  /** A glyph in a small box (a type's mark) or an icon name. */
  glyph?: string | undefined;
  icon?: IconName | undefined;
  /** A quiet note at the end ("one option"). */
  hint?: string | undefined;
  /** A choice that is on (a sort direction, the chosen type). */
  checked?: boolean | undefined;
  danger?: boolean | undefined;
  disabled?: boolean | undefined;
  onSelect: () => void;
}

/** One action of a menu: a real button, so Enter and Space work as usual. */
export function MenuItem({
  children,
  glyph,
  icon,
  hint,
  checked,
  danger = false,
  disabled = false,
  onSelect,
}: MenuItemProps) {
  const role = checked === undefined ? "menuitem" : "menuitemradio";
  return (
    <button
      type="button"
      role={role}
      aria-checked={checked}
      disabled={disabled}
      className={cx(styles["item"], danger ? styles["danger"] : undefined)}
      onClick={onSelect}
    >
      {glyph === undefined ? null : (
        <span className={styles["glyph"]} aria-hidden="true">
          {glyph}
        </span>
      )}
      {icon === undefined ? null : <Icon name={icon} />}
      <span className={styles["label"]}>{children}</span>
      {hint === undefined ? null : <span className={styles["hint"]}>{hint}</span>}
      {checked === true ? <Icon name="check" /> : null}
    </button>
  );
}

/** A small heading over a group of items ("Basic"). */
export function MenuHeading({ children }: { children: ReactNode }) {
  return (
    <div role="presentation" className={styles["heading"]}>
      {children}
    </div>
  );
}

/** A line between groups of items. */
export function MenuSeparator() {
  return <div role="separator" className={styles["separator"]} />;
}

export interface CheckGroupProps {
  /** Names the group ("Columns"). */
  legend: string;
  children: ReactNode;
}

/** A group of checkboxes under a name. */
export function CheckGroup({ legend, children }: CheckGroupProps) {
  return (
    <fieldset className={styles["group"]}>
      <legend className={styles["legend"]}>{legend}</legend>
      {children}
    </fieldset>
  );
}
