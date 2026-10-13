import type { ComponentPropsWithRef, ReactNode } from "react";

import { cx } from "../../internal/cx";

import styles from "./Chip.module.css";

export interface ChipProps extends Omit<ComponentPropsWithRef<"button">, "children"> {
  /** Toggle state for filter chips; omit for a plain action chip. */
  pressed?: boolean | undefined;
  children: ReactNode;
}

/** A 44px touch chip, for filters and quick choices on phones. */
export function Chip({ pressed, className, children, type = "button", ...rest }: ChipProps) {
  return (
    <button {...rest} type={type} className={cx(styles["chip"], className)} aria-pressed={pressed}>
      {children}
    </button>
  );
}

/** A horizontally scrolling row of chips. */
export function ChipRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles["row"]} role="group" aria-label={label}>
      {children}
    </div>
  );
}
