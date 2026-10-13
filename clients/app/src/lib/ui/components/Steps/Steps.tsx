import type { ReactNode } from "react";

import styles from "./Steps.module.css";

export interface StepsProps {
  /** Accessible name of the list ("What happens next"). */
  label: string;
  /** One sentence per step, in order. */
  steps: readonly ReactNode[];
}

/** What will happen, in order: a short numbered list (signing up, setting up). */
export function Steps({ label, steps }: StepsProps) {
  return (
    <ol className={styles["steps"]} aria-label={label}>
      {steps.map((step, index) => (
        <li key={index} className={styles["step"]}>
          <span className={styles["number"]} aria-hidden="true">
            {index + 1}
          </span>
          <span className={styles["text"]}>{step}</span>
        </li>
      ))}
    </ol>
  );
}
