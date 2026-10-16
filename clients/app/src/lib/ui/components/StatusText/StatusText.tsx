import type { ReactNode } from "react";

import styles from "./StatusText.module.css";

/** How something is doing: fine, sorting itself out, or needs someone. */
export type StatusTone = "ok" | "waiting" | "problem";

export interface StatusTextProps {
  tone: StatusTone;
  /** The status in words ("Syncing"); the dot only repeats it. */
  children: ReactNode;
}

/** A short status line: a coloured dot and the words that carry the meaning. */
export function StatusText({ tone, children }: StatusTextProps) {
  return (
    <span className={styles["status"]} data-tone={tone}>
      <span className={styles["dot"]} aria-hidden="true" />
      <span className={styles["text"]}>{children}</span>
    </span>
  );
}
