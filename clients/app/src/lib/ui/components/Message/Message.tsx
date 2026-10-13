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
  /** Who it went to: "To Sam Rivera, cc Leo Brandt". */
  recipients?: string | undefined;
  /**
   * Plain text, shown as a paragraph. With `rich`, any block content (a
   * {@link SafeHtml} frame for an HTML body): untrusted HTML renders only
   * through lib/ui/safe-html, never here.
   */
  children: ReactNode;
  rich?: boolean | undefined;
  attachment?: string | undefined;
  /** Several attached files. */
  attachments?: readonly string[] | undefined;
  /** Actions for this one message ("Show images"), under the body. */
  actions?: ReactNode | undefined;
  /** Its files with their actions (an `AttachmentList`), last. */
  files?: ReactNode | undefined;
}

/** One message of a conversation. */
export function MessageCard({
  from,
  avatar,
  time,
  dateTime,
  recipients,
  children,
  rich = false,
  attachment,
  attachments = [],
  actions,
  files: fileList,
}: MessageCardProps) {
  const files = attachment === undefined ? attachments : [attachment, ...attachments];
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
        {recipients === undefined ? null : <p className={styles["recipients"]}>{recipients}</p>}
        {rich ? (
          <div className={styles["rich"]}>{children}</div>
        ) : (
          <p className={styles["text"]}>{children}</p>
        )}
        {actions === undefined ? null : <div className={styles["actions"]}>{actions}</div>}
        {files.length === 0 ? null : (
          <div className={styles["files"]}>
            {files.map((name, index) => (
              <Attachment key={`${name}-${String(index)}`} name={name} />
            ))}
          </div>
        )}
        {fileList}
      </div>
    </article>
  );
}

export interface FoldedMessageProps {
  from: string;
  avatar?: Pick<AvatarProps, "kind" | "initials"> | undefined;
  time: string;
  dateTime?: string | undefined;
  /** The start of its text. */
  snippet: string;
  /** Unfold it. */
  onOpen: () => void;
}

/** An older message of a thread, folded to one line until opened. */
export function FoldedMessage({
  from,
  avatar,
  time,
  dateTime,
  snippet,
  onOpen,
}: FoldedMessageProps) {
  return (
    <button type="button" className={styles["folded"]} onClick={onOpen} aria-expanded={false}>
      <Avatar name={from} size="sm" {...avatar} />
      <b className={styles["from"]}>{from}</b>
      <span className={styles["foldedSnippet"]}>{snippet}</span>
      <time className={styles["time"]} dateTime={dateTime}>
        {time}
      </time>
    </button>
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
