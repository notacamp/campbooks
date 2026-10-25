import type { MouseEventHandler, ReactNode } from "react";

import { Icon, type IconName } from "../Icon/Icon";

import styles from "./RichDocument.module.css";

export interface RichDocumentProps {
  /** The editor's content element (or the same markup, read-only). */
  children: ReactNode;
}

/**
 * The look of a page's body: headings, lists, to-dos, quotes, callouts, code,
 * dividers, mentions and cards, drawn from the tokens. The editor's own
 * markup goes inside; nothing here knows about the editor.
 */
export function RichDocument({ children }: RichDocumentProps) {
  return <div className={styles["document"]}>{children}</div>;
}

/** What a mention points at. */
export type MentionKind = "person" | "thread" | "task" | "event" | "page";

const MENTION_ICONS = {
  person: "people",
  thread: "mail",
  task: "tasks",
  event: "event",
  page: "document",
} as const satisfies Record<MentionKind, IconName>;

export interface MentionChipProps {
  kind: MentionKind;
  /** Its current name: the person, the subject, the title. */
  label: string;
  /** Where it opens; omit for what the viewer cannot open or is still loading. */
  href?: string | undefined;
  /** Open it in the app without a reload. */
  onOpen?: MouseEventHandler<HTMLAnchorElement> | undefined;
  /** `closed`: something the viewer cannot open, said in plain words and no more. */
  state?: "ready" | "loading" | "closed" | undefined;
}

/** An inline pointer to a record, as a chip in the text. */
export function MentionChip({ kind, label, href, onOpen, state = "ready" }: MentionChipProps) {
  const icon = state === "closed" ? "lock" : MENTION_ICONS[kind];
  const content = (
    <>
      <Icon name={icon} />
      <span className={styles["chipLabel"]}>{label}</span>
    </>
  );
  return href !== undefined && state === "ready" ? (
    <a className={styles["chip"]} href={href} onClick={onOpen} data-kind={kind}>
      {content}
    </a>
  ) : (
    <span className={styles["chip"]} data-kind={kind} data-state={state}>
      {content}
    </span>
  );
}

export interface DocCardProps {
  icon: IconName;
  title: string;
  /** A line under the title (a file's size, a thread's sender). */
  meta?: string | undefined;
  href?: string | undefined;
  onOpen?: MouseEventHandler<HTMLAnchorElement> | undefined;
  /** For a file: the name it downloads as. */
  download?: string | undefined;
  /** `closed`: something the viewer cannot open. */
  state?: "ready" | "loading" | "closed" | undefined;
  children?: ReactNode | undefined;
}

/** A block that shows a file, a record or a sub-page as a card. */
export function DocCard({
  icon,
  title,
  meta,
  href,
  onOpen,
  download,
  state = "ready",
  children,
}: DocCardProps) {
  const body = (
    <>
      <Icon name={state === "closed" ? "lock" : icon} size="lg" />
      <span className={styles["cardText"]}>
        <span className={styles["cardTitle"]}>{title}</span>
        {meta === undefined ? null : <span className={styles["cardMeta"]}>{meta}</span>}
      </span>
    </>
  );
  return (
    <div className={styles["card"]} data-state={state}>
      {href !== undefined && state === "ready" ? (
        <a className={styles["cardLink"]} href={href} onClick={onOpen} download={download}>
          {body}
        </a>
      ) : (
        <span className={styles["cardLink"]}>{body}</span>
      )}
      {children}
    </div>
  );
}

export interface DocImageProps {
  src: string;
  alt: string;
  /** The image could not be loaded or the viewer cannot open it. */
  state?: "ready" | "closed" | undefined;
  caption?: string | undefined;
}

/** An image block. */
export function DocImage({ src, alt, state = "ready", caption }: DocImageProps) {
  return (
    <figure className={styles["figure"]} data-state={state}>
      {state === "closed" ? (
        <span className={styles["figureClosed"]}>
          <Icon name="lock" size="lg" />
          {alt}
        </span>
      ) : (
        <img className={styles["image"]} src={src} alt={alt} loading="lazy" />
      )}
      {caption === undefined ? null : <figcaption>{caption}</figcaption>}
    </figure>
  );
}

export interface DocToggleProps {
  open: boolean;
  /** Said by the arrow: "Show what is inside" / "Hide what is inside". */
  label: string;
  onToggle: () => void;
  /** The summary and what is inside; only the first line shows while closed. */
  children: ReactNode;
}

/** A toggle block: an arrow, and a first line that hides what follows it. */
export function DocToggle({ open, label, onToggle, children }: DocToggleProps) {
  return (
    <div className={styles["toggle"]} data-open={open}>
      <button
        type="button"
        className={styles["toggleArrow"]}
        aria-expanded={open}
        aria-label={label}
        contentEditable={false}
        onClick={onToggle}
      >
        <Icon name={open ? "chevron-down" : "chevron-right"} />
      </button>
      <div className={styles["toggleBody"]}>{children}</div>
    </div>
  );
}
