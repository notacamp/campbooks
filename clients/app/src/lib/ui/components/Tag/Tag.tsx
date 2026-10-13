import type { ReactNode } from "react";

import { Icon, type IconName } from "../Icon/Icon";

import styles from "./Tag.module.css";

export interface TagProps {
  icon?: IconName | undefined;
  /** `danger` for a fact that is late (an overdue date). */
  tone?: "neutral" | "danger" | undefined;
  children: ReactNode;
}

/** A neutral outlined label for facts about an item (attachments, amounts, a due date). */
export function Tag({ icon, tone = "neutral", children }: TagProps) {
  return (
    <span className={styles["tag"]} data-tone={tone === "neutral" ? undefined : tone}>
      {icon === undefined ? null : <Icon name={icon} />}
      <span className={styles["text"]}>{children}</span>
    </span>
  );
}
