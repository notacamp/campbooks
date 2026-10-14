import type { CSSProperties, ReactNode } from "react";

import styles from "./Progress.module.css";

export interface ProgressProps {
  /** What is being done, for screen readers ("Moving 1,000 to Updates"). */
  label: string;
  /** How much is done. */
  value: number;
  /**
   * How much there is. 0 (or less) when that is not known yet: the bar then
   * shows work going on without claiming how far it has got.
   */
  max: number;
  /** The line under the bar ("340 of 1,000 done"), announced politely as it changes. */
  children?: ReactNode | undefined;
}

/** How far some work has got: a bar and a line of text. */
export function Progress({ label, value, max, children }: ProgressProps) {
  const known = max > 0;
  const done = known ? Math.min(1, Math.max(0, value / max)) : 0;
  return (
    <div className={styles["progress"]}>
      <div
        className={
          known ? styles["track"] : `${styles["track"] ?? ""} ${styles["indeterminate"] ?? ""}`
        }
        role="progressbar"
        aria-label={label}
        {...(known
          ? { "aria-valuemin": 0, "aria-valuemax": max, "aria-valuenow": Math.min(value, max) }
          : { "aria-busy": true })}
      >
        <span className={styles["fill"]} style={{ "--progress": done } as CSSProperties} />
      </div>
      {children === undefined ? null : (
        <p className={styles["line"]} aria-live="polite">
          {children}
        </p>
      )}
    </div>
  );
}
