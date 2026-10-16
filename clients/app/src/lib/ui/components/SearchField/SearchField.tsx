import type { ComponentPropsWithRef, ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { ShortcutId } from "~/lib/shortcuts";

import { cx } from "../../internal/cx";
import { Icon } from "../Icon/Icon";
import { KbdHint } from "../Kbd/Kbd";

import styles from "./SearchField.module.css";

/** A filter the search understood, shown under the field and removable. */
export interface SearchFieldChip {
  id: string;
  /** "From ana", "Has attachment". */
  label: string;
  onRemove: () => void;
}

export interface SearchFieldProps extends Omit<
  ComponentPropsWithRef<"input">,
  "value" | "onChange" | "size" | "children"
> {
  /** Accessible name of the field. */
  label: string;
  value: string;
  onValueChange: (value: string) => void;
  chips?: readonly SearchFieldChip[] | undefined;
  /** Shows the clear button while there is text. */
  onClear?: (() => void) | undefined;
  /** The keys that reach the field, hinted at its end (from the registry). */
  shortcut?: ShortcutId | undefined;
  /** Or one-off keys in binding syntax ("escape"). */
  keys?: string | undefined;
  /** `overlay` sits at the top of a dialog; `page` is a boxed field. */
  variant?: "overlay" | "page" | undefined;
  /** Under the chips (quick filters). */
  children?: ReactNode | undefined;
}

/**
 * The search box: a field, the filters it understood as removable chips
 * (wrapping on phones), a clear button and a hint of the keys that reach it.
 * Extra input attributes (combobox roles, `aria-activedescendant`, key
 * handlers) go to the input.
 */
export function SearchField({
  label,
  value,
  onValueChange,
  chips = [],
  onClear,
  shortcut,
  keys,
  variant = "page",
  className,
  children,
  ...rest
}: SearchFieldProps) {
  const { t } = useTranslation("ui");
  return (
    <div className={cx(styles["field"], className)} data-variant={variant}>
      <div className={styles["row"]}>
        <Icon name="search" />
        <input
          {...rest}
          className={styles["input"]}
          type="search"
          enterKeyHint="search"
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          aria-label={label}
          value={value}
          onChange={(event) => {
            onValueChange(event.target.value);
          }}
        />
        {onClear !== undefined && value !== "" ? (
          <button type="button" className={styles["clear"]} onClick={onClear}>
            <Icon name="close" />
            <span className={styles["hidden"]}>{t("searchField.clear")}</span>
          </button>
        ) : null}
        {shortcut === undefined && keys === undefined ? null : (
          <KbdHint shortcut={shortcut} keys={keys} />
        )}
      </div>
      <SearchFieldChips chips={chips} />
      {children}
    </div>
  );
}

/** The understood filters as removable chips (wrapping on phones). */
export function SearchFieldChips({ chips }: { chips: readonly SearchFieldChip[] }) {
  const { t } = useTranslation("ui");
  if (chips.length === 0) {
    return null;
  }
  return (
    <ul className={styles["chips"]} aria-label={t("searchField.filters")}>
      {chips.map((chip) => (
        <li key={chip.id} className={styles["chip"]}>
          <span className={styles["chipLabel"]}>{chip.label}</span>
          <button
            type="button"
            className={styles["remove"]}
            onClick={chip.onRemove}
            aria-label={t("searchField.remove", { filter: chip.label })}
          >
            <Icon name="close" />
          </button>
        </li>
      ))}
    </ul>
  );
}
