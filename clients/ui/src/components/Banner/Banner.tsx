import type { ReactNode } from "react";

import { Icon, type IconName } from "../Icon/Icon";

import styles from "./Banner.module.css";

export interface BannerProps {
  /** neutral: informational (offline). danger: something needs fixing (a mailbox stopped syncing). */
  tone?: "neutral" | "danger" | undefined;
  icon?: IconName | undefined;
  /** The bold first sentence. */
  title: ReactNode;
  /** The rest of the message. */
  children?: ReactNode | undefined;
  /** One action, usually a secondary Button ("Reconnect"). */
  action?: ReactNode | undefined;
  /** `alert` when it appears because of something the user is doing. */
  role?: "status" | "alert" | undefined;
}

/** A message about the state of the app, inline above the content it affects. */
export function Banner({ tone = "neutral", icon, title, children, action, role }: BannerProps) {
  return (
    <div className={styles["banner"]} data-tone={tone} role={role}>
      <Icon name={icon ?? (tone === "danger" ? "warning" : "offline")} />
      <p className={styles["message"]}>
        <b className={styles["title"]}>{title}</b> {children}
      </p>
      {action}
    </div>
  );
}
