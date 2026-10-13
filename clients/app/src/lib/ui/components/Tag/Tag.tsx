import type { ReactNode } from "react";

import { Icon, type IconName } from "../Icon/Icon";

import styles from "./Tag.module.css";

export interface TagProps {
  icon?: IconName | undefined;
  children: ReactNode;
}

/** A neutral outlined label for facts about an item (attachments, amounts). */
export function Tag({ icon, children }: TagProps) {
  return (
    <span className={styles["tag"]}>
      {icon === undefined ? null : <Icon name={icon} />}
      <span className={styles["text"]}>{children}</span>
    </span>
  );
}
