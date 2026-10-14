import type { CSSProperties, ReactNode } from "react";

import styles from "./Progress.module.css";

export interface ProgressProps {
  /** What is being done, for screen readers ("Moving 1,000 to Updates"). */
  label: string;
  /** How much is done. */
  value: number;
  /** How much there is; 0 shows an empty bar. */
  max: number;
  /** The line under the bar ("340 of 1,000 done"). */
  children?: ReactNode | undefined;
}

/** How far some work has got: a bar and a line of text. */
export function Progress({ label, value, max, children }: ProgressProps) {
  const done = max <= 0 ? 0 : Math.min(1, Math.max(0, value / max));
  return (
    <div className={styles["progress"]}>
      <div
        className={styles["track"]}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={Math.min(value, max)}
      >
        <span className={styles["fill"]} style={{ "--progress": done } as CSSProperties} />
      </div>
      {children === undefined ? null : <p className={styles["line"]}>{children}</p>}
    </div>
  );
}
