import type { ReactNode } from "react";

import styles from "./List.module.css";

/** A plain list of rows (members, invitations), named for screen readers. */
export function List({ label, children }: { label: string; children: ReactNode }) {
  return (
    <ul className={styles["list"]} aria-label={label}>
      {children}
    </ul>
  );
}

export interface ListRowProps {
  /** An avatar or icon. */
  leading?: ReactNode | undefined;
  /** The row's name ("Sofia Pessoa"). */
  title: ReactNode;
  /** Next to the title: a short label ("You"). */
  badge?: ReactNode | undefined;
  /** The muted second line (an email, a date). */
  meta?: ReactNode | undefined;
  /** Controls at the end (a role picker, Remove). Wrap below on phones. */
  trailing?: ReactNode | undefined;
  /** Shown under the row, full width (an inline confirmation, an error). */
  children?: ReactNode | undefined;
  /** Dimmed: something no longer active (an expired invitation). */
  muted?: boolean | undefined;
}

/** One row of a {@link List}. */
export function ListRow({ leading, title, badge, meta, trailing, children, muted }: ListRowProps) {
  return (
    <li className={styles["row"]} data-muted={muted === true || undefined}>
      <div className={styles["main"]}>
        {leading}
        <div className={styles["text"]}>
          <span className={styles["titleLine"]}>
            <span className={styles["title"]}>{title}</span>
            {badge === undefined ? null : <span className={styles["badge"]}>{badge}</span>}
          </span>
          {meta === undefined ? null : <span className={styles["meta"]}>{meta}</span>}
        </div>
        {trailing === undefined ? null : <div className={styles["trailing"]}>{trailing}</div>}
      </div>
      {children === undefined || children === null ? null : (
        <div className={styles["below"]}>{children}</div>
      )}
    </li>
  );
}
