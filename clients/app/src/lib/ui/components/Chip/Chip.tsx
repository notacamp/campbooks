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

export interface ChipLinkProps extends Omit<ComponentPropsWithRef<"a">, "children"> {
  /** This is the page you are on (`aria-current="page"`). */
  current?: boolean | undefined;
  children: ReactNode;
}

/** A chip that goes somewhere (a page's sections on phones): a real link. */
export function ChipLink({ current, className, children, ...rest }: ChipLinkProps) {
  return (
    <a
      {...rest}
      className={cx(styles["chip"], className)}
      aria-current={current === true ? "page" : undefined}
    >
      {children}
    </a>
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
