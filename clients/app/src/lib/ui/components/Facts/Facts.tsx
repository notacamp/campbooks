import type { ReactNode } from "react";

import styles from "./Facts.module.css";

/** A named group of facts (a description list) in a raised panel, like a {@link List}. */
export function FactList({ children }: { children: ReactNode }) {
  return <dl className={styles["list"]}>{children}</dl>;
}

/** How a fact is doing; the dot repeats what the value says, never replaces it. */
export type FactStatus = "ok" | "problem" | "off";

export interface FactProps {
  /** What the fact is ("Database"). */
  label: ReactNode;
  /** Its value ("Reachable"). */
  children: ReactNode;
  /** A coloured dot before the value: fine, needs fixing, or not in use. */
  status?: FactStatus | undefined;
  /** A muted line under the value (a model, an address, a date). */
  detail?: ReactNode | undefined;
}

/** One row of a {@link FactList}: the label, then its value (stacked on phones). */
export function Fact({ label, children, status, detail }: FactProps) {
  return (
    <div className={styles["row"]}>
      <dt className={styles["label"]}>{label}</dt>
      <dd className={styles["value"]}>
        <span className={styles["main"]}>
          {status === undefined ? null : (
            <span className={styles["dot"]} data-status={status} aria-hidden="true" />
          )}
          <span className={styles["text"]}>{children}</span>
        </span>
        {detail === undefined ? null : <span className={styles["detail"]}>{detail}</span>}
      </dd>
    </div>
  );
}
