import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Avatar, type AvatarProps } from "../Avatar/Avatar";
import { Icon } from "../Icon/Icon";
import { VisuallyHidden } from "../Typography/Typography";

import styles from "./ConversationRow.module.css";

export interface ConversationRowProps {
  from: string;
  avatar?: Pick<AvatarProps, "kind" | "initials"> | undefined;
  /** "Fri", "09:05". */
  time: string;
  /** ISO timestamp for the <time> element. */
  dateTime?: string | undefined;
  subject: string;
  snippet: string;
  unread?: boolean | undefined;
  starred?: boolean | undefined;
  /** Messages in the conversation; shown from 2. */
  count?: number | undefined;
  /** Some message has a file attached. */
  attachment?: boolean | undefined;
  /** A line under the snippet: the assistant's suggestion ("Scout suggests Newsletters"). */
  suggestion?: ReactNode | undefined;
  /** The conversation open in the reader. */
  current?: boolean | undefined;
  /** The keyboard cursor (J/K) is on it. */
  selected?: boolean | undefined;
  /** Element id, so the list can scroll the cursor into view. */
  id?: string | undefined;
  onSelect: () => void;
}

/** A row of a stream's conversation list. */
export function ConversationRow({
  from,
  avatar,
  time,
  dateTime,
  subject,
  snippet,
  unread = false,
  starred = false,
  count,
  attachment = false,
  suggestion,
  current = false,
  selected = false,
  id,
  onSelect,
}: ConversationRowProps) {
  const { t } = useTranslation("ui");
  return (
    <button
      type="button"
      id={id}
      className={styles["row"]}
      data-unread={unread || undefined}
      data-selected={selected || undefined}
      aria-current={current ? "true" : undefined}
      onClick={onSelect}
    >
      <Avatar name={from} {...avatar} />
      <span className={styles["from"]}>
        <span className={styles["fromName"]}>{from}</span>
        {count !== undefined && count > 1 ? <span className={styles["count"]}>{count}</span> : null}
        {unread ? <VisuallyHidden>{t("conversation.unread")}</VisuallyHidden> : null}
      </span>{" "}
      <span className={styles["trailing"]}>
        {attachment ? (
          <span className={styles["flag"]} title={t("conversation.attachment")}>
            <Icon name="attachment" />
            <VisuallyHidden>{t("conversation.attachment")}</VisuallyHidden>
          </span>
        ) : null}
        {starred ? (
          <span className={styles["flag"]} data-starred="true" title={t("conversation.starred")}>
            <Icon name="star-filled" />
            <VisuallyHidden>{t("conversation.starred")}</VisuallyHidden>
          </span>
        ) : null}
        <time className={styles["time"]} dateTime={dateTime}>
          {time}
        </time>
      </span>{" "}
      <span className={styles["subject"]}>{subject}</span>{" "}
      <span className={styles["snippet"]}>{snippet}</span>
      {suggestion === undefined ? null : <span className={styles["suggestion"]}>{suggestion}</span>}
    </button>
  );
}
