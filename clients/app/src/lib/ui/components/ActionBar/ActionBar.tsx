import type { ReactNode } from "react";

import styles from "./ActionBar.module.css";

export interface ActionBarProps {
  /** Accessible name of the group ("Conversation actions"). */
  label: string;
  /** Large (44px) buttons; give the main one `grow`. */
  children: ReactNode;
}

/** A floating bar of a screen's main actions, pinned above a phone's home indicator. */
export function ActionBar({ label, children }: ActionBarProps) {
  return (
    <div className={styles["bar"]} role="group" aria-label={label}>
      {children}
    </div>
  );
}
