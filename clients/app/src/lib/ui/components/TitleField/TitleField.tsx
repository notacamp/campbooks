import type { ComponentPropsWithRef } from "react";

import { cx } from "../../internal/cx";

import styles from "./TitleField.module.css";

export interface TitleFieldProps extends Omit<ComponentPropsWithRef<"input">, "type" | "size"> {
  /** Accessible name ("Page title"); the field has no visible label. */
  label: string;
}

/** A page's title, edited in place: a big heading that is an input. */
export function TitleField({ label, className, ...rest }: TitleFieldProps) {
  return (
    <input
      {...rest}
      type="text"
      className={cx(styles["title"], className)}
      aria-label={label}
      autoComplete="off"
      spellCheck
    />
  );
}
