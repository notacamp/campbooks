import { Avatar, type AvatarProps } from "../Avatar/Avatar";

import styles from "./ConversationRow.module.css";

export interface ConversationRowProps {
  from: string;
  avatar?: Pick<AvatarProps, "kind" | "initials"> | undefined;
  /** "Fri", "09:05". */
  time: string;
  subject: string;
  snippet: string;
  unread?: boolean | undefined;
  /** The conversation open in the reader. */
  current?: boolean | undefined;
  onSelect: () => void;
}

/** A row of a stream's conversation list. */
export function ConversationRow({
  from,
  avatar,
  time,
  subject,
  snippet,
  unread = false,
  current = false,
  onSelect,
}: ConversationRowProps) {
  return (
    <button
      type="button"
      className={styles["row"]}
      data-unread={unread || undefined}
      aria-current={current ? "true" : undefined}
      onClick={onSelect}
    >
      <Avatar name={from} {...avatar} />
      <span className={styles["from"]}>
        <span className={styles["fromName"]}>{from}</span>
      </span>{" "}
      <span className={styles["time"]}>{time}</span>{" "}
      <span className={styles["subject"]}>{subject}</span>{" "}
      <span className={styles["snippet"]}>{snippet}</span>
    </button>
  );
}
