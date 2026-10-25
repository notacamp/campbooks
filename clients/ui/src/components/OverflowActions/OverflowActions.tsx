import { useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactElement } from "react";

import type { ShortcutId } from "../../shortcuts";

import { Button } from "../Button/Button";
import type { IconName } from "../Icon/Icon";
import { IconButton } from "../IconButton/IconButton";
import { Popover } from "../Popover/Popover";

import styles from "./OverflowActions.module.css";

export interface OverflowAction {
  /** Stable key. */
  id: string;
  /** The action's name: the icon's tooltip, the menu row's text. */
  label: string;
  /** An icon name, or any glyph (the visor, for an action that asks Scout). */
  icon: IconName | ReactElement;
  /** Its key, shown in the tooltip and in the menu row. */
  shortcut?: ShortcutId | undefined;
  /** A toggle's state (Pin), as aria-pressed on the icon. */
  pressed?: boolean | undefined;
  onSelect: () => void;
}

export interface OverflowActionsProps {
  /** Names the menu's button and panel ("More actions"). */
  label: string;
  /** Icon actions, in order, from the start of the free space. */
  actions: OverflowAction[];
  /** Actions at the end of the row (Previous, Next), folded with the rest. */
  end?: OverflowAction[] | undefined;
  /**
   * Show `actions` as labelled buttons (name and key) instead of icons, for a
   * row of a few words-first choices (Scout noticed). A glyph icon (the visor)
   * goes before the name; an icon name shows in the menu only.
   */
  buttons?: boolean | undefined;
  /** Actions that always live in the More menu, after the rest. */
  more?: OverflowAction[] | undefined;
}

/**
 * The rest of a toolbar row: icon actions (or labelled buttons) while they
 * fit, else one "More" menu holding them all, each with its name and key, so
 * the row never wraps. `more` actions are always in the menu.
 * It takes the row's free space (it is a flex item that grows), so whatever
 * sits before it keeps its place; the actions' keys work either way.
 */
export function OverflowActions({
  label,
  actions,
  end = [],
  buttons = false,
  more = [],
}: OverflowActionsProps) {
  const row = useRef<HTMLDivElement>(null);
  const [folded, setFolded] = useState(false);
  // The width the icons took when last shown: they come back once it fits.
  const needed = useRef(0);
  const check = useRef(() => {
    // Replaced on mount.
  });

  useLayoutEffect(() => {
    const box = row.current;
    if (box === null || typeof ResizeObserver === "undefined") {
      return;
    }
    check.current = () => {
      if (box.dataset["folded"] === undefined) {
        if (box.scrollWidth > box.clientWidth + 1) {
          needed.current = box.scrollWidth;
          setFolded(true);
        }
      } else if (box.clientWidth >= needed.current) {
        setFolded(false);
      }
    };
    const observer = new ResizeObserver(() => {
      check.current();
    });
    observer.observe(box);
    return () => {
      observer.disconnect();
    };
  }, []);

  // Different actions take a different width: show them and measure again.
  const count = actions.length + end.length + more.length;
  const [measuredCount, setMeasuredCount] = useState(count);
  if (measuredCount !== count) {
    setMeasuredCount(count);
    setFolded(false);
  }
  useLayoutEffect(() => {
    check.current();
  });

  const menu = (items: OverflowAction[]) => (
    <Popover
      label={label}
      renderTrigger={(trigger) => <IconButton {...trigger} label={label} icon="more" />}
    >
      {(close) => (
        <div className={styles["menu"]}>
          {items.map((action) => (
            <Button
              key={action.id}
              variant="ghost"
              icon={typeof action.icon === "string" ? action.icon : undefined}
              leading={typeof action.icon === "string" ? undefined : action.icon}
              shortcut={action.shortcut}
              onKeyDown={moveFocus}
              onClick={() => {
                close();
                action.onSelect();
              }}
            >
              {action.label}
            </Button>
          ))}
        </div>
      )}
    </Popover>
  );

  if (folded) {
    return (
      <div ref={row} className={styles["row"]} data-folded="">
        <span className={styles["spacer"]} />
        {menu([...actions, ...end, ...more])}
      </div>
    );
  }

  const icon = (action: OverflowAction) => (
    <IconButton
      key={action.id}
      label={action.label}
      icon={action.icon}
      shortcut={action.shortcut}
      aria-pressed={action.pressed}
      onClick={action.onSelect}
    />
  );
  const button = (action: OverflowAction) => (
    <Button
      key={action.id}
      variant="secondary"
      leading={typeof action.icon === "string" ? undefined : action.icon}
      shortcut={action.shortcut}
      aria-pressed={action.pressed}
      onClick={action.onSelect}
    >
      {action.label}
    </Button>
  );
  return (
    <div ref={row} className={styles["row"]} data-buttons={buttons || undefined}>
      {actions.map(buttons ? button : icon)}
      {more.length === 0 ? null : menu(more)}
      <span className={styles["spacer"]} />
      {end.map(icon)}
    </div>
  );
}

/** ↑/↓ move between the menu's rows (the row's siblings). */
function moveFocus(event: KeyboardEvent<HTMLButtonElement>) {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
    return;
  }
  const row = event.currentTarget;
  const rows = [
    ...(row.parentElement?.querySelectorAll<HTMLButtonElement>(":scope > button") ?? []),
  ];
  const at = rows.indexOf(row);
  const next = rows[(at + (event.key === "ArrowDown" ? 1 : rows.length - 1)) % rows.length];
  event.preventDefault();
  event.stopPropagation();
  next?.focus();
}
