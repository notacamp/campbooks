import type { ReactNode } from "react";

import { Icon } from "../Icon/Icon";

import styles from "./Disclosure.module.css";

export interface DisclosureProps {
  /** What opening it shows ("What Campbooks reads and keeps"). */
  summary: ReactNode;
  /** The detail: a few short paragraphs. */
  children: ReactNode;
  /** Start open. */
  defaultOpen?: boolean | undefined;
}

/**
 * Detail one click (or Enter) away, so a page can keep to one question while
 * still saying everything: a native `<details>`, keyboard and screen reader
 * behaviour included.
 */
export function Disclosure({ summary, children, defaultOpen = false }: DisclosureProps) {
  return (
    <details className={styles["disclosure"]} open={defaultOpen || undefined}>
      <summary className={styles["summary"]}>
        <Icon name="chevron-down" className={styles["chevron"]} />
        <span>{summary}</span>
      </summary>
      <div className={styles["body"]}>{children}</div>
    </details>
  );
}
