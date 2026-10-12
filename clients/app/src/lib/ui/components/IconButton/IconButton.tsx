import type { ComponentPropsWithRef, ReactElement } from "react";

import {
  ariaKeyShortcut,
  formatBinding,
  useShortcutDefinition,
  useShortcuts,
  type ShortcutId,
} from "~/lib/shortcuts";

import { cx } from "../../internal/cx";
import { Icon, type IconName } from "../Icon/Icon";

import styles from "./IconButton.module.css";

export interface IconButtonProps extends Omit<
  ComponentPropsWithRef<"button">,
  "children" | "aria-label"
> {
  /** Accessible name; also shown as the tooltip. Required: there is no visible text. */
  label: string;
  /** An icon name, or any glyph (e.g. a Visor). */
  icon: IconName | ReactElement;
  /** md follows density; lg is the 44px touch size. */
  size?: "md" | "lg" | undefined;
  /** The shortcut it triggers: exposed as aria-keyshortcuts and in the tooltip. */
  shortcut?: ShortcutId | undefined;
}

export function IconButton({
  label,
  icon,
  size = "md",
  shortcut,
  className,
  type = "button",
  ...rest
}: IconButtonProps) {
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition(shortcut ?? "");
  const keys =
    definition === undefined
      ? undefined
      : formatBinding(definition.keys, platform)
          .map((chord) => chord.join(""))
          .join(" ");
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles["iconButton"], className)}
      data-size={size}
      aria-label={label}
      title={keys === undefined ? label : `${label} (${keys})`}
      aria-keyshortcuts={
        definition === undefined ? undefined : ariaKeyShortcut(definition.keys, platform)
      }
    >
      {typeof icon === "string" ? <Icon name={icon} size={size === "lg" ? "lg" : "md"} /> : icon}
    </button>
  );
}
