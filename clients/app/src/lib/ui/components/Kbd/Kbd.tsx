import { Fragment, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import {
  bindingsOf,
  formatBinding,
  useShortcutDefinition,
  useShortcuts,
  type ShortcutDefinition,
  type ShortcutId,
} from "~/lib/shortcuts";

import { cx } from "../../internal/cx";
import { VisuallyHidden } from "../Typography/Typography";

import styles from "./Kbd.module.css";

/** One keycap. */
export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className={styles["kbd"]}>{children}</kbd>;
}

export interface KbdHintProps {
  /** Draw the keys bound to this shortcut, read from the registry. */
  shortcut?: ShortcutId | undefined;
  /** Or explicit keys in binding syntax ("1", "mod+enter") for one-off hints. */
  keys?: string | undefined;
  /** Write "then" between the steps of a sequence (G then I). */
  withThen?: boolean | undefined;
  /** Draw the shortcut's other bindings too: "J or ↓". */
  withAlternates?: boolean | undefined;
  /** Read the keys out to screen readers. Off by default: the control the
   *  hint sits in carries `aria-keyshortcuts` instead. */
  announce?: boolean | undefined;
  /** Only show while the parent row is hovered, focused or selected. */
  reveal?: boolean | undefined;
  /** `sm`: smaller, quieter keycaps, for compact controls (the calendar's views). */
  size?: "md" | "sm" | undefined;
  className?: string | undefined;
}

function bindingsToDraw(
  keys: string | undefined,
  definition: ShortcutDefinition | undefined,
  withAlternates: boolean,
): string[] {
  if (keys !== undefined) {
    return [keys];
  }
  if (definition === undefined) {
    return [];
  }
  return withAlternates ? bindingsOf(definition) : [definition.keys];
}

/**
 * Keyboard hint for an action. With `shortcut`, the keys come from the
 * shortcut registry, so the hint always matches the binding.
 */
export function KbdHint({
  shortcut,
  keys,
  withThen = false,
  withAlternates = false,
  announce = false,
  reveal = false,
  size = "md",
  className,
}: KbdHintProps) {
  const { t } = useTranslation("ui");
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition(shortcut ?? "");
  const bindings = bindingsToDraw(keys, definition, withAlternates);
  if (bindings.length === 0) {
    return null;
  }
  const drawn = bindings.map((binding) => formatBinding(binding, platform));
  const spoken = drawn
    .map((chords) => chords.map((chord) => chord.join(" ")).join(` ${t("kbd.then")} `))
    .join(` ${t("kbd.or")} `);
  return (
    <span
      className={cx(styles["hint"], className)}
      data-reveal={reveal || undefined}
      data-announce={announce || undefined}
      data-size={size === "sm" ? "sm" : undefined}
    >
      <span className={styles["keys"]} aria-hidden="true">
        {drawn.map((chords, alternative) => (
          <Fragment key={alternative}>
            {alternative > 0 ? <span className={styles["then"]}>{t("kbd.or")}</span> : null}
            {chords.map((chord, index) => (
              <Fragment key={index}>
                {index > 0 && withThen ? (
                  <span className={styles["then"]}>{t("kbd.then")}</span>
                ) : null}
                {chord.map((key) => (
                  <Kbd key={key}>{key}</Kbd>
                ))}
              </Fragment>
            ))}
          </Fragment>
        ))}
      </span>
      {announce ? <VisuallyHidden>{t("kbd.shortcut", { keys: spoken })}</VisuallyHidden> : null}
    </span>
  );
}
