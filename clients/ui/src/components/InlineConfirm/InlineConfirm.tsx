import { useEffect, useId, useRef, type ReactNode } from "react";

import { Button } from "../Button/Button";

import styles from "./InlineConfirm.module.css";

export interface InlineConfirmProps {
  /** What will happen, in one or two sentences ("Remove Ana? They lose access now."). */
  children: ReactNode;
  /** The action's name ("Remove"). */
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  /** While the action runs: buttons disabled. */
  busy?: boolean | undefined;
}

/**
 * Confirms a risky action in place, next to what it affects, instead of a
 * modal. Focus moves to Cancel; Escape cancels.
 */
export function InlineConfirm({
  children,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  busy = false,
}: InlineConfirmProps) {
  const messageId = useId();
  const root = useRef<HTMLDivElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const onCancelRef = useRef(onCancel);
  useEffect(() => {
    onCancelRef.current = onCancel;
  });

  useEffect(() => {
    cancel.current?.focus();
    const element = root.current;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        event.preventDefault();
        onCancelRef.current();
      }
    };
    element?.addEventListener("keydown", onKeyDown);
    return () => {
      element?.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  return (
    <div ref={root} className={styles["confirm"]} role="group" aria-labelledby={messageId}>
      <p id={messageId} className={styles["message"]}>
        {children}
      </p>
      <div className={styles["actions"]}>
        <Button variant="danger" disabled={busy} onClick={onConfirm}>
          {confirmLabel}
        </Button>
        <Button ref={cancel} variant="ghost" disabled={busy} onClick={onCancel}>
          {cancelLabel}
        </Button>
      </div>
    </div>
  );
}
