import { useEffect, useRef, type ReactNode } from "react";

import type { Signal } from "../../tokens/tokens";
import { Avatar, type AvatarProps } from "../Avatar/Avatar";
import { Icon, type IconName } from "../Icon/Icon";
import { Pill } from "../Pill/Pill";
import { Tag } from "../Tag/Tag";
import { Visor } from "../Visor/Visor";

import styles from "./Item.module.css";

export interface ItemProps {
  /** Who it is from: "Sofia Pessoa · Brightloop". */
  who: string;
  /** What it needs, in one line. */
  what: string;
  /** The sender's own words, on one line under it while closed (quoted, plain text). */
  excerpt?: string | undefined;
  avatar: Pick<AvatarProps, "name" | "kind" | "initials">;
  /** When: "Thu", "Today", "Fri · €148.00". */
  due?: string | undefined;
  /** Due soon: drawn in the danger colour. */
  soon?: boolean | undefined;
  /**
   * How long it has waited, above the due date: "3 hours ago", "Yesterday",
   * "12 Oct". `dateTime` is the machine-readable instant and `label` the full
   * date, shown on hover.
   */
  age?: { text: string; dateTime: string; label: string } | undefined;
  /** Facts shown as tags under the line. */
  tags?: string[] | undefined;
  /** Facts in a signal's colour, before the tags ("Held Wed 10:00–10:30" in Do's). */
  pills?: readonly { text: string; signal: Signal }[] | undefined;
  /** The keyboard cursor is here (J/K). */
  selected?: boolean | undefined;
  /** The detail is open below the row. */
  expanded?: boolean | undefined;
  onToggle?: (() => void) | undefined;
  /** Phone rows open a page instead of expanding. */
  href?: string | undefined;
  /**
   * The selected row's actions, right under it (an {@link ItemActions}).
   * Pass them for the selected row only: other rows stay calm.
   */
  actions?: ReactNode | undefined;
  /**
   * The row was acted on: it becomes this one-line confirmation ("Done ·
   * Marta Ferreira") and folds away, then `onConfirmEnd` runs (under a
   * second; an instant swap under reduced motion).
   */
  confirmation?: string | undefined;
  onConfirmEnd?: (() => void) | undefined;
  /** Shown under the row while expanded (usually an {@link ItemDetail}). */
  detail?: ReactNode | undefined;
  /** Revealed actions, as after a swipe on a phone ({@link SwipeAction}s). */
  swipeActions?: ReactNode | undefined;
  id?: string | undefined;
}

/**
 * A row of the "needs you" queue. A real button (or link) that expands to
 * the quote, the assistant's context and the actions.
 */
export function Item({
  who,
  what,
  excerpt,
  avatar,
  due,
  soon = false,
  age,
  tags = [],
  pills = [],
  selected = false,
  expanded = false,
  onToggle,
  href,
  actions,
  confirmation,
  onConfirmEnd,
  detail,
  swipeActions,
  id,
}: ItemProps) {
  if (confirmation !== undefined) {
    return <ItemConfirmation id={id} text={confirmation} onEnd={onConfirmEnd} />;
  }
  const content = (
    <>
      <Avatar {...avatar} />
      <span className={styles["who"]}>{who}</span>{" "}
      <span className={styles["when"]}>
        {age === undefined ? null : (
          <time className={styles["age"]} dateTime={age.dateTime} title={age.label}>
            {age.text}
          </time>
        )}
        {due === undefined ? null : (
          <span className={styles["due"]} data-soon={soon || undefined}>
            {due}
          </span>
        )}
      </span>{" "}
      <span className={styles["what"]}>{what}</span>
      {excerpt === undefined || expanded ? null : <q className={styles["excerpt"]}>{excerpt}</q>}
      {tags.length + pills.length > 0 ? (
        <span className={styles["meta"]}>
          {pills.map((pill) => (
            <Pill key={pill.text} signal={pill.signal}>
              {pill.text}
            </Pill>
          ))}
          {tags.map((tag) => (
            <Tag key={tag}>{tag}</Tag>
          ))}
        </span>
      ) : null}
    </>
  );
  const shared = {
    id,
    className: styles["item"],
    "data-selected": selected || undefined,
  };
  const row =
    href === undefined ? (
      <button
        {...shared}
        type="button"
        aria-expanded={detail === undefined ? undefined : expanded}
        onClick={onToggle}
      >
        {content}
      </button>
    ) : (
      <a {...shared} href={href}>
        {content}
      </a>
    );
  return (
    <div
      className={styles["wrapper"]}
      data-selected={selected || undefined}
      data-expanded={expanded && detail !== undefined ? "" : undefined}
    >
      {swipeActions === undefined ? (
        row
      ) : (
        <div className={styles["swipe"]}>
          {row}
          {swipeActions}
        </div>
      )}
      {actions === undefined ? null : <div className={styles["actions"]}>{actions}</div>}
      {expanded && detail !== undefined ? detail : null}
    </div>
  );
}

/** How long a confirmation shows before it has folded away (`--duration-confirm`). */
const ITEM_CONFIRM_MS = 900;

/** A row acted on: one line, a check, then it folds away. */
function ItemConfirmation({
  id,
  text,
  onEnd,
}: {
  id: string | undefined;
  text: string;
  onEnd: (() => void) | undefined;
}) {
  const end = useRef(onEnd);
  useEffect(() => {
    end.current = onEnd;
  });
  useEffect(() => {
    const timer = setTimeout(() => {
      end.current?.();
    }, ITEM_CONFIRM_MS);
    return () => {
      clearTimeout(timer);
    };
  }, []);
  return (
    <div id={id} className={styles["confirmation"]}>
      <span className={styles["check"]} aria-hidden="true">
        <Icon name="check" />
      </span>
      <span className={styles["confirmationText"]}>{text}</span>
    </div>
  );
}

export interface ItemDetailProps {
  /** "Sofia, Friday 17:42". */
  quoteFrom?: string | undefined;
  /** The sender's own words: untrusted mail text, shown quoted and as plain text. */
  quote?: string | undefined;
  /** Why to be careful, before the quote (a warning's tells). */
  reasons?: string[] | undefined;
  /**
   * Scout's summary of the email, in one or two plain sentences: a
   * model's words about untrusted mail, shown as text only, marked with the
   * visor, after the reasons and above the email. Nothing is drawn without it.
   */
  summary?: string | undefined;
  /** The summary's heading: "Scout's summary". */
  summaryLabel?: string | undefined;
  /** The email itself, after the summary (in place of the quote). */
  mail?: ReactNode | undefined;
  /** The assistant's context, next to the visor. */
  context?: string | undefined;
  /** Buttons: the item's own actions, then Later and Done. */
  actions: ReactNode;
  /** "How Campbooks decided" ({@link HowDecided}). */
  how?: ReactNode | undefined;
}

/** The expanded part of an {@link Item}. */
export function ItemDetail({
  quoteFrom,
  quote,
  reasons = [],
  summary,
  summaryLabel,
  mail,
  context,
  actions,
  how,
}: ItemDetailProps) {
  return (
    <div className={styles["detail"]}>
      {reasons.length === 0 ? null : (
        <ul className={styles["reasons"]}>
          {reasons.map((reason) => (
            <li key={reason}>{reason}</li>
          ))}
        </ul>
      )}
      {summary === undefined || summary === "" ? null : (
        <section className={styles["summary"]} aria-label={summaryLabel}>
          {summaryLabel === undefined ? null : (
            <span className={styles["summaryHead"]} aria-hidden="true">
              <Visor size="xs" />
              {summaryLabel}
            </span>
          )}
          <p className={styles["summaryText"]}>{summary}</p>
        </section>
      )}
      {mail}
      {quote === undefined || mail !== undefined ? null : (
        <p className={styles["quote"]}>
          {quoteFrom === undefined ? null : (
            <span className={styles["quoteFrom"]}>{quoteFrom}</span>
          )}
          <q className={styles["quoteText"]}>{quote}</q>
        </p>
      )}
      {context === undefined ? null : (
        <p className={styles["context"]}>
          <Visor />
          {context}
        </p>
      )}
      <div className={styles["actions"]}>{actions}</div>
      {how}
    </div>
  );
}

export interface SwipeActionProps {
  tone: "later" | "done";
  icon: IconName;
  label: string;
  onClick: () => void;
}

/** One action revealed behind a swiped row. */
export function SwipeAction({ tone, icon, label, onClick }: SwipeActionProps) {
  return (
    <button type="button" className={styles["swipeAction"]} data-tone={tone} onClick={onClick}>
      <Icon name={icon} />
      {label}
    </button>
  );
}
