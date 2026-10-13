import { Fragment, useId, useState, type ReactNode } from "react";
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

export interface NoteProps {
  /** The assistant's sentence about where things stand. */
  children: ReactNode;
  /** The visor's mood; `unsure` when confidence is low. */
  visor?: VisorState | undefined;
  /** Compact text and visor, for phones. */
  compact?: boolean | undefined;
  /** Records this conversation is linked to (task, document, event). */
  links?: NoteLink[] | undefined;
  /** "Sorted into Primary, 98% sure". */
  confidence?: string | undefined;
  /** Reasons behind the sorting, shown under "Why?". */
  why?: string[] | undefined;
  /** When unsure, the assistant asks instead of acting. */
  choice?: NoteChoice | undefined;
  /** Extra actions in the footer (e.g. "Move"). */
  footer?: ReactNode | undefined;
  /** Accessible name of the region. Defaults to "Where things stand". */
  label?: string | undefined;
}

/** The assistant's "where things stand" block: the hero of a conversation. */
export function Note({
  children,
  visor = "idle",
  compact = false,
  links = [],
  confidence,
  why = [],
  choice,
  footer,
  label,
}: NoteProps) {
  const { t } = useTranslation("ui");
  const [whyOpen, setWhyOpen] = useState(false);
  const whyId = useId();
  const footItems: ReactNode[] = [];
  if (confidence !== undefined) {
    footItems.push(<span>{confidence}</span>);
  }
  if (why.length > 0) {
    footItems.push(
      <Button
        variant="link"
        aria-expanded={whyOpen}
        aria-controls={whyId}
        onClick={() => {
          setWhyOpen((open) => !open);
        }}
      >
        {whyOpen ? t("note.hideWhy") : t("note.why")}
      </Button>,
    );
  }
  if (footer !== undefined) {
    footItems.push(footer);
  }

  return (
    <section
      className={styles["note"]}
      aria-label={label ?? t("note.label")}
      data-compact={compact || undefined}
    >
      <Visor state={visor} size={compact ? "md" : "lg"} />
      <div className={styles["body"]}>
        <p className={styles["text"]}>{children}</p>
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
        {footItems.length > 0 ? (
          <div className={styles["foot"]}>
            {footItems.map((item, index) => (
              <Fragment key={index}>
                {index > 0 ? <span aria-hidden="true">·</span> : null}
                {item}
              </Fragment>
            ))}
          </div>
        ) : null}
        {why.length > 0 ? (
          <ul id={whyId} className={styles["why"]} hidden={!whyOpen}>
            {why.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        ) : null}
      </div>
    </section>
  );
}
