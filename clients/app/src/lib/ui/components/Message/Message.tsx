import { Fragment, type ReactNode } from "react";

import { Avatar, type AvatarProps } from "../Avatar/Avatar";
import { Icon } from "../Icon/Icon";

import styles from "./Message.module.css";

export interface MessageCardProps {
  from: string;
  avatar?: Pick<AvatarProps, "kind" | "initials"> | undefined;
  /** "Fri 17:42". */
  time: string;
  /** ISO timestamp for the <time> element. */
  dateTime?: string | undefined;
  /** Plain text. Untrusted HTML bodies render through lib/ui/safe-html, never here. */
  children: ReactNode;
  attachment?: string | undefined;
}

/** One message of a conversation. */
export function MessageCard({
  from,
  avatar,
  time,
  dateTime,
  children,
  attachment,
}: MessageCardProps) {
  return (
    <article className={styles["message"]}>
      <Avatar name={from} {...avatar} />
      <div className={styles["body"]}>
        <div className={styles["head"]}>
          <b className={styles["from"]}>{from}</b>
          <time className={styles["time"]} dateTime={dateTime}>
            {time}
          </time>
        </div>
        <p className={styles["text"]}>{children}</p>
        {attachment === undefined ? null : <Attachment name={attachment} />}
      </div>
    </article>
  );
}

/** A file attached to a message; long names are cut with an ellipsis. */
export function Attachment({ name }: { name: string }) {
  return (
    <span className={styles["attachment"]} title={name}>
      <Icon name="attachment" />
      <span className={styles["attachmentName"]}>{name}</span>
    </span>
  );
}

/** A muted line of facts separated by middle dots: "Sofia Pessoa · Brightloop · 2 messages". */
export function MetaLine({ parts }: { parts: string[] }) {
  return (
    <p className={styles["meta"]}>
      {parts.map((part, index) => (
        <Fragment key={part}>
          {index > 0 ? <span aria-hidden="true">·</span> : null}
          <span>{part}</span>
        </Fragment>
      ))}
    </p>
  );
}
