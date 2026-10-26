import { useState } from "react";

import { IconButton } from "../IconButton/IconButton";

import styles from "./CodeBlock.module.css";

export interface CodeBlockProps {
  /** What the text is ("Last error"), for screen readers. */
  label: string;
  /** The text, shown as written: monospace, wrapped, never cut. */
  children: string;
  /** The copy button's name ("Copy error"). */
  copyLabel: string;
  /** Said once copied ("Copied"). */
  copiedLabel: string;
  /** Said when the clipboard refused ("Select the text and copy it"). */
  failedLabel: string;
  /** Copy `children`; defaults to the system clipboard. */
  copy?: ((value: string) => Promise<void>) | undefined;
}

const clipboardCopy = (value: string) => navigator.clipboard.writeText(value);

/**
 * Text to read and hand on (an error message, a log line): monospace,
 * wrapped so it never scrolls sideways on a phone, with a Copy button whose
 * result is announced. It grows with the text (an error is at most a couple
 * of thousand characters), so there is nothing to scroll.
 */
export function CodeBlock({
  label,
  children,
  copyLabel,
  copiedLabel,
  failedLabel,
  copy = clipboardCopy,
}: CodeBlockProps) {
  // The outcome belongs to the text it was for: new text starts afresh.
  const [result, setResult] = useState<{ value: string; copied: boolean } | null>(null);
  const status = result?.value === children ? (result.copied ? "copied" : "failed") : "idle";

  return (
    <div className={styles["block"]}>
      <pre className={styles["code"]} aria-label={label}>
        {children}
      </pre>
      <IconButton
        icon="copy"
        label={copyLabel}
        className={styles["copy"]}
        onClick={() => {
          copy(children).then(
            () => {
              setResult({ value: children, copied: true });
            },
            () => {
              setResult({ value: children, copied: false });
            },
          );
        }}
      />
      <p className={styles["status"]} role="status">
        {status === "copied" ? copiedLabel : status === "failed" ? failedLabel : null}
      </p>
    </div>
  );
}
