import { useId, useRef, useState } from "react";

import { useShortcut, type ShortcutId } from "~/lib/shortcuts";

import { Button } from "../Button/Button";

import styles from "./CopyField.module.css";

export interface CopyFieldProps {
  /** What the value is ("Invitation link"). */
  label: string;
  value: string;
  /** The button ("Copy link"). */
  copyLabel: string;
  /** Said once copied ("Copied"). */
  copiedLabel: string;
  /** Said when the clipboard refused ("Select the link and copy it"). */
  failedLabel: string;
  /** The shortcut that copies, shown on the button and bound while mounted. */
  shortcut?: ShortcutId | undefined;
  /** Copy `value`; defaults to the system clipboard. */
  copy?: ((value: string) => Promise<void>) | undefined;
}

const clipboardCopy = (value: string) => navigator.clipboard.writeText(value);

/**
 * A value to hand on (a link), shown read-only with a Copy button. The
 * result is announced; when the clipboard refuses, the text is selected so
 * the platform's own copy works.
 */
export function CopyField({
  label,
  value,
  copyLabel,
  copiedLabel,
  failedLabel,
  shortcut,
  copy = clipboardCopy,
}: CopyFieldProps) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  // The outcome belongs to the value it was for: a new value starts afresh.
  const [result, setResult] = useState<{ value: string; copied: boolean } | null>(null);
  const status = result?.value === value ? (result.copied ? "copied" : "failed") : "idle";

  const run = () => {
    copy(value).then(
      () => {
        setResult({ value, copied: true });
      },
      () => {
        input.current?.focus();
        input.current?.select();
        setResult({ value, copied: false });
      },
    );
  };
  useShortcut(shortcut ?? "", run, { enabled: shortcut !== undefined });

  return (
    <div className={styles["copy"]}>
      <label className={styles["label"]} htmlFor={id}>
        {label}
      </label>
      <div className={styles["row"]}>
        <input
          ref={input}
          id={id}
          className={styles["value"]}
          readOnly
          value={value}
          onFocus={(event) => {
            event.currentTarget.select();
          }}
        />
        <Button variant="primary" icon="copy" shortcut={shortcut} onClick={run}>
          {copyLabel}
        </Button>
      </div>
      <p className={styles["status"]} role="status">
        {status === "copied" ? copiedLabel : status === "failed" ? failedLabel : null}
      </p>
    </div>
  );
}
