import { useId, useState, type ReactNode } from "react";

import { Button, IconButton, Skeleton, Visor } from "@notacamp/ui";

import styles from "./Explanation.module.css";

/** A message a point comes from: "Jonas · 29 Oct". It opens that message. */
export interface ExplanationChip {
  id: string;
  label: string;
  onSelect: () => void;
}

/** One point of an explanation: plain text, never markup, and where it comes from. */
export interface ExplanationItem {
  id: string;
  /** Who it is about ("Jonas Weber"), said before the text. */
  lead?: string | undefined;
  /** The point, as plain text. */
  text: string;
  /** When it is due, said after the text ("by Thu 6 Nov"). */
  due?: string | undefined;
  chips: ExplanationChip[];
}

/** A list of points under a heading: who wants what, agreed, still open, on you. */
export interface ExplanationGroup {
  id: string;
  label: string;
  items: ExplanationItem[];
  /** "On you": set apart with the accent, not in alarm colours. */
  emphasis?: boolean | undefined;
  /** On a phone it is behind the "more" button until opened. */
  folded?: boolean | undefined;
}

export interface ExplanationCardProps {
  /** "Scout's explanation": the card's name. */
  title: string;
  /** "From 9 messages · written today 10:42". */
  meta?: string | undefined;
  /** Hide: folds the card to its one line. */
  hideLabel: string;
  onHide: () => void;
  /** Said above the content: how long the thread is, that it is out of date, why it failed. */
  notice?: ReactNode | undefined;
  /** While Scout reads: "Scout is reading the 9 messages…", with Stop. */
  writing?:
    { label: string; stopLabel: string; onStop: () => void; inShortLabel: string } | undefined;
  /** When it is written. */
  inShort?: { label: string; text: string } | undefined;
  groups?: ExplanationGroup[] | undefined;
  /** Phone only: "Who wants what, agreed, open" and its opposite. */
  more?: { label: string; lessLabel: string } | undefined;
  /** Ask a follow-up, Reply, Explain again. */
  actions?: ReactNode | undefined;
  /** "Only you see this. ..." */
  privacy?: string | undefined;
}

/**
 * Scout's explanation of a thread, below the "Where things stand" note:
 * the thread in short, then who wants what, what was agreed, what is open and
 * what is on you, each point with the messages it comes from. Everything is
 * plain text. While Scout reads it is a skeleton; on a phone only the summary
 * and what is on you show until the rest is opened.
 */
export function ExplanationCard({
  title,
  meta,
  hideLabel,
  onHide,
  notice,
  writing,
  inShort,
  groups = [],
  more,
  actions,
  privacy,
}: ExplanationCardProps) {
  const [open, setOpen] = useState(false);
  const moreId = useId();
  const folds = groups.some((group) => group.folded === true);
  return (
    <section
      className={styles["card"]}
      aria-label={title}
      aria-busy={writing === undefined ? undefined : true}
    >
      <div className={styles["head"]}>
        <Visor state={writing === undefined ? "idle" : "thinking"} size="xs" />
        <span className={styles["title"]}>{writing === undefined ? title : writing.label}</span>
        {meta === undefined || writing !== undefined ? null : (
          <span className={styles["meta"]}>{meta}</span>
        )}
        <span className={styles["spacer"]} />
        {writing === undefined ? (
          <IconButton label={hideLabel} icon="chevron-up" onClick={onHide} />
        ) : (
          <Button variant="ghost" onClick={writing.onStop}>
            {writing.stopLabel}
          </Button>
        )}
      </div>
      {notice === undefined ? null : (
        <div className={styles["notice"]} role="status">
          {notice}
        </div>
      )}
      {writing === undefined ? null : (
        <div className={styles["block"]}>
          <span className={styles["label"]}>{writing.inShortLabel}</span>
          <Skeleton width="88%" />
          <Skeleton width="62%" />
          <Skeleton width="78%" />
          <Skeleton width="54%" />
        </div>
      )}
      {inShort === undefined ? null : (
        <div className={styles["block"]}>
          <span className={styles["label"]}>{inShort.label}</span>
          <p className={styles["short"]}>{inShort.text}</p>
        </div>
      )}
      {groups.length === 0 ? null : (
        <div className={styles["groups"]} id={moreId} data-open={open || undefined}>
          {groups.map((group) => (
            <div
              key={group.id}
              className={styles["group"]}
              data-emphasis={group.emphasis === true || undefined}
              data-folded={group.folded === true || undefined}
            >
              <span className={styles["label"]}>{group.label}</span>
              {group.items.map((item) => (
                <p key={item.id} className={styles["item"]}>
                  <span className={styles["text"]}>
                    {item.lead === undefined ? null : (
                      <strong className={styles["lead"]}>{item.lead}</strong>
                    )}
                    {item.lead === undefined ? null : ": "}
                    {item.text}
                    {item.due === undefined ? null : ` ${item.due}`}
                  </span>
                  {item.chips.map((chip) => (
                    <button
                      key={chip.id}
                      type="button"
                      className={styles["chip"]}
                      onClick={chip.onSelect}
                    >
                      {chip.label}
                    </button>
                  ))}
                </p>
              ))}
            </div>
          ))}
        </div>
      )}
      {more === undefined || !folds ? null : (
        <button
          type="button"
          className={styles["more"]}
          aria-expanded={open}
          aria-controls={moreId}
          onClick={() => {
            setOpen((current) => !current);
          }}
        >
          {open ? more.lessLabel : more.label}
        </button>
      )}
      {actions === undefined && privacy === undefined ? null : (
        <div className={styles["foot"]}>
          {actions === undefined ? null : <span className={styles["actions"]}>{actions}</span>}
          {privacy === undefined ? null : <span className={styles["privacy"]}>{privacy}</span>}
        </div>
      )}
    </section>
  );
}

export interface ExplanationLineProps {
  /** "Your explanation from 10:42 · from 9 messages". */
  children: ReactNode;
  /** The buttons: Show, Explain again. */
  actions: ReactNode;
}

/** The one line a thread shows when you asked about it before: nothing opens until you choose. */
export function ExplanationLine({ children, actions }: ExplanationLineProps) {
  return (
    <div className={styles["line"]}>
      <Visor state="idle" size="xs" />
      <span className={styles["lineText"]}>{children}</span>
      <span className={styles["actions"]}>{actions}</span>
    </div>
  );
}
