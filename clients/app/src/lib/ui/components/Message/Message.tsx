import { Fragment, useId, useState, type ReactNode } from "react";

import { Avatar, type AvatarProps } from "../Avatar/Avatar";
import { Icon } from "../Icon/Icon";

import styles from "./Message.module.css";

/**
 * Whose message it is in the thread: theirs sit flush left, yours (sent from
 * the inbox) are set in from the left on the sunken surface.
 */
export type MessageSide = "theirs" | "yours";

export interface MessageCardProps {
  from: string;
  /** After the sender, for a message of yours: "→ Marta Silva". */
  addressee?: string | undefined;
  side?: MessageSide | undefined;
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
  /** Quoted history folded under the text (a {@link QuotedText}). */
  quoted?: ReactNode | undefined;
  /** What was read from it (the calendar's "Scout noticed" strip), under the header. */
  found?: ReactNode | undefined;
}

/** One message of a conversation. */
export function MessageCard({
  from,
  addressee,
  side = "theirs",
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
  quoted,
  found,
}: MessageCardProps) {
  const files = attachment === undefined ? attachments : [attachment, ...attachments];
  return (
    <article className={styles["message"]} data-side={side}>
      <Avatar name={from} {...avatar} />
      <div className={styles["body"]}>
        <div className={styles["head"]}>
          <b className={styles["from"]}>
            {from}
            {addressee === undefined || addressee === "" ? null : (
              <>
                {" "}
                <span className={styles["addressee"]}>{addressee}</span>
              </>
            )}
          </b>
          <time className={styles["time"]} dateTime={dateTime}>
            {time}
          </time>
        </div>
        {recipients === undefined ? null : <p className={styles["recipients"]}>{recipients}</p>}
        {found}
        {rich ? (
          <div className={styles["rich"]}>{children}</div>
        ) : (
          <p className={styles["text"]}>{children}</p>
        )}
        {quoted}
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
  /** After the sender, for a message of yours: "→ Marta Silva". */
  addressee?: string | undefined;
  side?: MessageSide | undefined;
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
  addressee,
  side = "theirs",
  avatar,
  time,
  dateTime,
  snippet,
  onOpen,
}: FoldedMessageProps) {
  return (
    <button
      type="button"
      className={styles["folded"]}
      data-side={side}
      onClick={onOpen}
      aria-expanded={false}
    >
      <Avatar name={from} size="sm" {...avatar} />
      <b className={styles["from"]}>
        {from}
        {addressee === undefined || addressee === "" ? null : (
          <>
            {" "}
            <span className={styles["addressee"]}>{addressee}</span>
          </>
        )}
      </b>
      <span className={styles["foldedSnippet"]}>{snippet}</span>
      <time className={styles["time"]} dateTime={dateTime}>
        {time}
      </time>
    </button>
  );
}

export interface MoreMessagesProps {
  /** "4 more messages". */
  label: string;
  /** Show them, folded, in its place. */
  onOpen: () => void;
}

/** The middle of a long thread, folded into one line until shown. */
export function MoreMessages({ label, onOpen }: MoreMessagesProps) {
  return (
    <button type="button" className={styles["more"]} onClick={onOpen} aria-expanded={false}>
      {label}
    </button>
  );
}

export interface QuotedTextProps {
  /** "Show quoted text". */
  showLabel: string;
  /** "Hide quoted text". */
  hideLabel: string;
  /** The quoted lines, as plain text. */
  children: string;
}

/** A plain-text message's quoted history, folded behind a toggle. */
export function QuotedText({ showLabel, hideLabel, children }: QuotedTextProps) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <div className={styles["quoted"]}>
      <button
        type="button"
        className={styles["quotedToggle"]}
        aria-expanded={open}
        aria-controls={id}
        onClick={() => {
          setOpen((current) => !current);
        }}
      >
        {open ? hideLabel : showLabel}
      </button>
      <p id={id} className={styles["quotedText"]} hidden={!open}>
        {children}
      </p>
    </div>
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
