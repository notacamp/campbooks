import type { ReactNode } from "react";

import styles from "./ActionBar.module.css";

export interface ActionBarProps {
  /** Accessible name of the group ("Conversation actions"). */
  label: string;
  /** Large (44px) buttons; give the main one `grow`. */
  children: ReactNode;
  /** Only below the medium breakpoint, where a pane header has no room for them. */
  phoneOnly?: boolean | undefined;
  /** Float above the phone's tab bar instead of the home indicator. */
  aboveTabBar?: boolean | undefined;
}

/** A floating bar of a screen's main actions, pinned above a phone's home indicator. */
export function ActionBar({
  label,
  children,
  phoneOnly = false,
  aboveTabBar = false,
}: ActionBarProps) {
  return (
    <div
      className={styles["bar"]}
      role="group"
      aria-label={label}
      data-phone-only={phoneOnly || undefined}
      data-above-tab-bar={aboveTabBar || undefined}
    >
      {children}
    </div>
  );
}
