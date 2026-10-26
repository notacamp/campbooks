import { useEffect, useId, useRef, useState, type ReactNode } from "react";

import { Button, Visor } from "@notacamp/ui";

import styles from "./ReplyDraft.module.css";

export interface ReplyQuoteProps {
  /** Who said it and when: "Marta Ferreira, Mon 12 Oct". */
  from: string;
  /** Their own words: untrusted mail text, shown quoted, as plain text, a few lines at most. */
  text: string;
  /** Said under the words when they are not all shown or not all quoted. */
  note?: ReactNode | undefined;
}

/** The words being answered, quoted at the top of a reply dialog. */
export function ReplyQuote({ from, text, note }: ReplyQuoteProps) {
  return (
    <figure className={styles["quote"]}>
      <figcaption className={styles["quoteFrom"]}>{from}</figcaption>
      <blockquote className={styles["quoteText"]}>{text}</blockquote>
      {note === undefined ? null : <p className={styles["quoteNote"]}>{note}</p>}
    </figure>
  );
}

export interface AskLineProps {
  /** The field's name and placeholder: "Or say it in your words". */
  label: string;
  /** An example under the label, as a placeholder: "e.g. yes, but I'll be late". */
  placeholder?: string | undefined;
  /** The button: "Draft it", "Change". It runs the AI, so it carries the visor. */
  action: string;
  /** While Scout is writing: nothing can be asked. */
  disabled?: boolean | undefined;
  /** Enter (or ⌘↵) or the button, with the trimmed words; the field then empties. */
  onSubmit: (text: string) => void;
  /** Longest text the field takes, in characters. */
  maxLength?: number | undefined;
}

/** One line to ask Scout in your own words, with the button that sends it. */
export function AskLine({
  label,
  placeholder,
  action,
  disabled = false,
  onSubmit,
  maxLength,
}: AskLineProps) {
  const id = useId();
  const [value, setValue] = useState("");
  const words = value.trim();
  const submit = () => {
    if (words !== "" && !disabled) {
      onSubmit(words);
      setValue("");
    }
  };
  return (
    <form
      className={styles["ask"]}
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <label className={styles["askLabel"]} htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className={styles["askInput"]}
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        disabled={disabled}
        maxLength={maxLength}
        onChange={(event) => {
          setValue(event.target.value);
        }}
        onKeyDown={(event) => {
          // ⌘↵ here asks, as Enter does: it never reaches a page's Send.
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            event.stopPropagation();
            submit();
          }
        }}
      />
      <Button type="submit" ai disabled={disabled || words === ""}>
        {action}
      </Button>
    </form>
  );
}

export interface ReplyDraftProps {
  /** The text area's name: "Your reply". */
  label: string;
  /** The draft, as plain text. */
  value: string;
  onChange: (value: string) => void;
  /** Scout is rewriting it: the text is kept but can't be changed meanwhile. */
  busy?: boolean | undefined;
  /** What Scout is doing while busy: "Making it shorter…". */
  busyLabel?: string | undefined;
  /** Buttons under the text (Shorter, Back to the choices). */
  tools?: ReactNode | undefined;
}

/**
 * Scout's draft of a reply, editable in place, with the buttons that
 * rework it beneath. The text is only ever plain text in a text area, which
 * takes the focus when it appears and after each rewrite.
 */
export function ReplyDraft({
  label,
  value,
  onChange,
  busy = false,
  busyLabel,
  tools,
}: ReplyDraftProps) {
  const id = useId();
  const statusId = useId();
  const text = useRef<HTMLTextAreaElement>(null);
  // The text takes the focus when it appears, and again once a rewrite is
  // done (the button that started it is gone meanwhile).
  useEffect(() => {
    if (!busy) {
      text.current?.focus({ preventScroll: true });
    }
  }, [busy]);
  return (
    <div className={styles["draft"]}>
      <label className={styles["askLabel"]} htmlFor={id}>
        {label}
      </label>
      <textarea
        ref={text}
        id={id}
        className={styles["text"]}
        value={value}
        readOnly={busy}
        aria-busy={busy || undefined}
        aria-describedby={busy && busyLabel !== undefined ? statusId : undefined}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <div className={styles["tools"]}>
        {busy && busyLabel !== undefined ? (
          <span id={statusId} className={styles["busy"]} role="status">
            <Visor state="thinking" size="xs" />
            {busyLabel}
          </span>
        ) : (
          tools
        )}
      </div>
    </div>
  );
}
