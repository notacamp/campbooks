import type { MouseEventHandler } from "react";

import {
  ariaKeyShortcut,
  useShortcutDefinition,
  useShortcuts,
  type ShortcutId,
} from "~/lib/shortcuts";

import { Icon, type IconName } from "../Icon/Icon";
import { KbdHint } from "../Kbd/Kbd";

import styles from "./NavItem.module.css";

export interface NavItemProps {
  label: string;
  icon?: IconName | undefined;
  /** Unread / open count. */
  count?: number | undefined;
  /** The shortcut that goes here (G then I); hinted on sidebar hover. */
  shortcut?: ShortcutId | undefined;
  /** Or one-off keys ("1" for the first stream). */
  keys?: string | undefined;
  /** This is the page you are on. */
  current?: boolean | undefined;
  /** A stream nested under its place. */
  sub?: boolean | undefined;
  href?: string | undefined;
  onClick?: MouseEventHandler<HTMLElement> | undefined;
}

/** A sidebar destination. A link when it has an href, else a button. */
export function NavItem({
  label,
  icon,
  count,
  shortcut,
  keys,
  current = false,
  sub = false,
  href,
  onClick,
}: NavItemProps) {
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition(shortcut ?? "");
  const binding = keys ?? definition?.keys;
  const content = (
    <>
      {icon === undefined ? null : <Icon name={icon} />}
      <span className={styles["label"]}>{label}</span>{" "}
      {count === undefined ? null : <span className={styles["count"]}>{count}</span>}
      {binding === undefined ? null : <KbdHint keys={binding} reveal className={styles["hint"]} />}
    </>
  );
  const shared = {
    className: styles["nav"],
    "data-sub": sub || undefined,
    "aria-current": current ? ("page" as const) : undefined,
    "aria-keyshortcuts": binding === undefined ? undefined : ariaKeyShortcut(binding, platform),
    onClick,
  };
  return href === undefined ? (
    <button {...shared} type="button">
      {content}
    </button>
  ) : (
    <a {...shared} href={href}>
      {content}
    </a>
  );
}
