import { useState } from "react";

import { TextField } from "~/lib/ui";

export interface DateCommitFieldProps {
  id?: string | undefined;
  label: string;
  /** The day it shows ("2026-10-26"), or "". */
  value: string;
  /** The earliest day that counts; an earlier one is dropped, not sent. */
  min?: string | undefined;
  /** The day was settled: the field was left, or Enter was pressed. */
  onCommit: (day: string) => void;
}

/** A whole day, with a year that cannot be a half-typed one. */
const complete = (day: string) =>
  /^\d{4}-\d{2}-\d{2}$/.test(day) && Number(day.slice(0, 4)) >= 1970;

/**
 * A date field that settles on blur or Enter, never on the way: typing the
 * year passes through 0002-10-26, and each of those must not become a change.
 */
export function DateCommitField({ id, label, value, min, onCommit }: DateCommitFieldProps) {
  const [draft, setDraft] = useState(value);
  const [seen, setSeen] = useState(value);
  if (seen !== value) {
    setSeen(value);
    setDraft(value);
  }
  const commit = () => {
    if (draft === value) {
      return;
    }
    if (!complete(draft) || (min !== undefined && draft < min)) {
      setDraft(value);
      return;
    }
    onCommit(draft);
  };
  return (
    <TextField
      {...(id === undefined ? {} : { id })}
      type="date"
      label={label}
      hideLabel
      {...(min === undefined ? {} : { min })}
      value={draft}
      onChange={(event) => {
        setDraft(event.target.value);
      }}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          commit();
        }
      }}
    />
  );
}
