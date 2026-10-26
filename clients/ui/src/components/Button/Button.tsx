import type { ComponentPropsWithRef, ReactNode } from "react";

import {
  ariaKeyShortcut,
  useShortcutDefinition,
  useShortcuts,
  type ShortcutId,
} from "../../shortcuts";

import { cx } from "../../internal/cx";
import { Icon, type IconName } from "../Icon/Icon";
import { KbdHint } from "../Kbd/Kbd";
import { Visor } from "../Visor/Visor";

import styles from "./Button.module.css";

export type ButtonVariant =
  "primary" | "secondary" | "ghost" | "danger" | "danger-outline" | "link";
/** md follows density (32px, 26px compact); lg is the 44px touch size. */
export type ButtonSize = "md" | "lg";

interface ButtonLookProps {
  variant?: ButtonVariant | undefined;
  size?: ButtonSize | undefined;
  /** Show this shortcut's keys (from the registry) and expose them as aria-keyshortcuts. */
  shortcut?: ShortcutId | undefined;
  /** Or a one-off key hint in binding syntax ("1", "2") for in-place choices. */
  keys?: string | undefined;
  icon?: IconName | undefined;
  /** Anything drawn before the label (a Visor, an Avatar). */
  leading?: ReactNode | undefined;
  /** The action runs the AI: the small visor goes before the label (never ✨). */
  ai?: boolean | undefined;
  /** Take the remaining width in a row. */
  grow?: boolean | undefined;
  children: ReactNode;
}

function useKeyShortcutAttribute(shortcut: ShortcutId | undefined, keys: string | undefined) {
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition(shortcut ?? "");
  const binding = keys ?? definition?.keys;
  return binding === undefined ? undefined : ariaKeyShortcut(binding, platform);
}

function Content({
  icon,
  leading,
  ai,
  children,
  shortcut,
  keys,
}: Pick<ButtonLookProps, "icon" | "leading" | "ai" | "children" | "shortcut" | "keys">) {
  return (
    <>
      {ai === true ? <Visor size="xs" /> : null}
      {leading}
      {icon === undefined ? null : <Icon name={icon} />}
      <span className={styles["label"]}>{children}</span>
      {shortcut === undefined && keys === undefined ? null : (
        <KbdHint shortcut={shortcut} keys={keys} />
      )}
    </>
  );
}

export type ButtonProps = ButtonLookProps & Omit<ComponentPropsWithRef<"button">, "children">;

/** A real <button>. Every action that has a shortcut shows it. */
export function Button({
  variant = "secondary",
  size = "md",
  shortcut,
  keys,
  icon,
  leading,
  ai = false,
  grow = false,
  className,
  children,
  type = "button",
  ...rest
}: ButtonProps) {
  const keyShortcut = useKeyShortcutAttribute(shortcut, keys);
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles["button"], className)}
      data-variant={variant}
      data-size={size}
      data-grow={grow || undefined}
      data-ai={ai || undefined}
      aria-keyshortcuts={rest["aria-keyshortcuts"] ?? keyShortcut}
    >
      <Content icon={icon} leading={leading} ai={ai} shortcut={shortcut} keys={keys}>
        {children}
      </Content>
    </button>
  );
}

export type ButtonLinkProps = ButtonLookProps & Omit<ComponentPropsWithRef<"a">, "children">;

/** A link that looks like a button (navigation, not an action). */
export function ButtonLink({
  variant = "secondary",
  size = "md",
  shortcut,
  keys,
  icon,
  leading,
  ai = false,
  grow = false,
  className,
  children,
  ...rest
}: ButtonLinkProps) {
  const keyShortcut = useKeyShortcutAttribute(shortcut, keys);
  return (
    <a
      {...rest}
      className={cx(styles["button"], className)}
      data-variant={variant}
      data-size={size}
      data-grow={grow || undefined}
      aria-keyshortcuts={keyShortcut}
    >
      <Content icon={icon} leading={leading} ai={ai} shortcut={shortcut} keys={keys}>
        {children}
      </Content>
    </a>
  );
}
