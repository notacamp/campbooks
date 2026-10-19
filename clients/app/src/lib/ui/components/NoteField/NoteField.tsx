import { useEffect, useId, useRef } from "react";

import styles from "./NoteField.module.css";

/** How close to the limit the counter shows, in characters. */
const NEAR_THE_LIMIT = 50;

export interface NoteFieldProps {
  /** The field's name: "About Marta, and how to write to them". */
  label: string;
  /** An example, as a placeholder: "e.g. My landlord. Formal, in Portuguese." */
  placeholder?: string | undefined;
  value: string;
  onChange: (value: string) => void;
  /** Longest text it takes, in characters. */
  maxLength: number;
  /** What the counter says near the limit: "12 characters left". */
  counter: (left: number) => string;
  /** ⌘↵ (Ctrl+Enter): keep it. It never reaches a dialog's or a page's own ⌘↵. */
  onSubmit: () => void;
  /** Esc: leave it. It never closes the dialog the field is in. */
  onCancel: () => void;
  /** While it is being kept: the text can't change. */
  disabled?: boolean | undefined;
  /** What went wrong, under the field. */
  error?: string | undefined;
}

/**
 * A short note in your own words: a labelled text area that takes the focus
 * when it appears, with a counter near its limit. ⌘↵ keeps it and Esc
 * leaves it, and both stop at the field, so a dialog around it stays open
 * and its own ⌘↵ (Send) is not run.
 */
export function NoteField({
  label,
  placeholder,
  value,
  onChange,
  maxLength,
  counter,
  onSubmit,
  onCancel,
  disabled = false,
  error,
}: NoteFieldProps) {
  const id = useId();
  const counterId = useId();
  const errorId = useId();
  const field = useRef<HTMLTextAreaElement>(null);
  const latest = useRef({ onSubmit, onCancel });
  useEffect(() => {
    latest.current = { onSubmit, onCancel };
  });

  useEffect(() => {
    const element = field.current;
    if (element === null) {
      return;
    }
    element.focus({ preventScroll: true });
    // On the element itself, before the keys reach a dialog's listener.
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        latest.current.onCancel();
      } else if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        event.stopPropagation();
        latest.current.onSubmit();
      }
    };
    element.addEventListener("keydown", onKeyDown);
    return () => {
      element.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  const left = maxLength - [...value].length;
  const near = left <= NEAR_THE_LIMIT;
  const described = [near ? counterId : null, error === undefined ? null : errorId]
    .filter((part) => part !== null)
    .join(" ");
  return (
    <div className={styles["field"]}>
      <label className={styles["label"]} htmlFor={id}>
        {label}
      </label>
      <textarea
        ref={field}
        id={id}
        className={styles["text"]}
        value={value}
        placeholder={placeholder}
        maxLength={maxLength}
        readOnly={disabled}
        aria-invalid={error === undefined ? undefined : true}
        aria-describedby={described === "" ? undefined : described}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {near ? (
        <p id={counterId} className={styles["counter"]}>
          {counter(Math.max(0, left))}
        </p>
      ) : null}
      {error === undefined ? null : (
        <p id={errorId} className={styles["error"]} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
