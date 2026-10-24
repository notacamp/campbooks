import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { ShortcutId } from "~/lib/shortcuts";

import { Button } from "../Button/Button";
import { Icon, type IconName } from "../Icon/Icon";
import { Visor, type VisorState } from "../Visor/Visor";

import styles from "./Note.module.css";

export interface NoteLink {
  icon: IconName;
  title: string;
  /** What it is: "Task, Thu", "Document". */
  kind?: string | undefined;
  onSelect: () => void;
}

export interface NoteChoiceOption {
  label: string;
  /** One-off key hint ("1", "2"), or a registered shortcut. */
  keys?: string | undefined;
  shortcut?: ShortcutId | undefined;
  onSelect: () => void;
}

export interface NoteChoice {
  /** "Where does this go?" */
  prompt: string;
  options: NoteChoiceOption[];
}

export interface NoteQuote {
  /** The sender's own sentence. Untrusted mail text: shown as quoted plain text only. */
  text: string;
  /** Who wrote it and when: "Marta Ferreira, Mon 12 Oct". */
  attribution: string;
}

export interface NoteProps {
  /** The assistant's sentence about where things stand. */
  children: ReactNode;
  /**
   * A title line above the sentence ("Scout noticed"). With one, the
   * sentence reads in the body font, for longer text; without, it stays the
   * short display-face line of a conversation.
   */
  title?: ReactNode | undefined;
  /** After the title, quieter ("1 of 5"). Shown only with a title. */
  meta?: ReactNode | undefined;
  /** Right under the sentence: a link to what it is about ("See the 34 emails"). */
  after?: ReactNode | undefined;
  /** The visor's mood; `unsure` when confidence is low, `alert` for a warning. */
  visor?: VisorState | undefined;
  /** Compact text and visor, for phones. */
  compact?: boolean | undefined;
  /** Why to be careful, before anything else (a phishing warning's tells). */
  reasons?: string[] | undefined;
  /** What the sender wrote, quoted. */
  quote?: NoteQuote | undefined;
  /** Short labels under the sentence ({@link Pill}s and {@link Tag}s). */
  chips?: ReactNode | undefined;
  /** Records this conversation is linked to (task, document, event). */
  links?: NoteLink[] | undefined;
  /** When unsure, the assistant asks instead of acting. */
  choice?: NoteChoice | undefined;
  /** Buttons for what the note is about (Done, Not needed). */
  actions?: ReactNode | undefined;
  /** "How Campbooks decided" ({@link HowDecided}): the only place numbers appear. */
  how?: ReactNode | undefined;
  /** Extra actions in the footer (e.g. "Move"). */
  footer?: ReactNode | undefined;
  /** Accessible name of the region. Defaults to "Where things stand". */
  label?: string | undefined;
}

/** The assistant's "where things stand" block: the hero of a conversation. */
export function Note({
  children,
  title,
  meta,
  after,
  visor = "idle",
  compact = false,
  reasons = [],
  quote,
  chips,
  links = [],
  choice,
  actions,
  how,
  footer,
  label,
}: NoteProps) {
  const { t } = useTranslation("ui");
  return (
    <section
      className={styles["note"]}
      aria-label={label ?? t("note.label")}
      data-compact={compact || undefined}
      data-titled={title === undefined ? undefined : ""}
    >
      <Visor state={visor} size={compact ? "md" : "lg"} />
      {title === undefined ? null : (
        <p className={styles["title"]}>
          <span className={styles["titleText"]}>{title}</span>
          {meta === undefined ? null : <span className={styles["meta"]}>{meta}</span>}
        </p>
      )}
      <div className={styles["body"]}>
        <p className={styles["text"]} data-body={title === undefined ? undefined : ""}>
          {children}
        </p>
        {after === undefined ? null : <div className={styles["after"]}>{after}</div>}
        {reasons.length === 0 ? null : (
          <ul className={styles["reasons"]}>
            {reasons.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        )}
        {quote === undefined ? null : (
          <blockquote className={styles["quote"]}>
            <q className={styles["quoteText"]}>{quote.text}</q>
            <footer className={styles["attribution"]}>{quote.attribution}</footer>
          </blockquote>
        )}
        {chips === undefined ? null : <div className={styles["chips"]}>{chips}</div>}
        {choice === undefined ? null : (
          <div className={styles["choice"]}>
            <span className={styles["prompt"]}>{choice.prompt}</span>
            {choice.options.map((option) => (
              <Button
                key={option.label}
                variant="secondary"
                onClick={option.onSelect}
                keys={option.keys}
                shortcut={option.shortcut}
              >
                {option.label}
              </Button>
            ))}
          </div>
        )}
        {links.length === 0 ? null : (
          <div className={styles["links"]}>
            {links.map((link) => (
              <button
                key={link.title}
                type="button"
                className={styles["link"]}
                onClick={link.onSelect}
              >
                <Icon name={link.icon} />
                <span className={styles["linkTitle"]}>{link.title}</span>
                {link.kind === undefined ? null : (
                  <span className={styles["linkKind"]}>{link.kind}</span>
                )}
              </button>
            ))}
          </div>
        )}
        {actions === undefined ? null : <div className={styles["actions"]}>{actions}</div>}
        {footer === undefined ? null : <div className={styles["foot"]}>{footer}</div>}
        {how === undefined ? null : <div className={styles["how"]}>{how}</div>}
      </div>
    </section>
  );
}
