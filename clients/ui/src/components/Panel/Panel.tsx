import type { ReactNode } from "react";

import { cx } from "../../internal/cx";

import styles from "./Panel.module.css";

export interface PanelProps {
  /** solid: a sunken block of its own · dashed: something kept elsewhere. */
  variant?: "solid" | "dashed" | undefined;
  /** What it is, for assistive technology. */
  label?: string | undefined;
  /** Lay the children out in a row that wraps, instead of a column. */
  row?: boolean | undefined;
  className?: string | undefined;
  children: ReactNode;
}

/** A sunken, bordered block set inside a page: a conflict, a block kept elsewhere. */
export function Panel({ variant = "solid", label, row = false, className, children }: PanelProps) {
  return (
    <section
      className={cx(styles["panel"], className)}
      data-variant={variant}
      data-row={row || undefined}
      aria-label={label}
    >
      {children}
    </section>
  );
}
