import { useId, type ComponentPropsWithRef, type ReactNode } from "react";

import styles from "./Checkbox.module.css";

export interface CheckboxProps extends Omit<ComponentPropsWithRef<"input">, "type" | "children"> {
  /** What ticking it means ("Also use this account to sign in"). */
  label: ReactNode;
  /** A muted sentence under the label (when to untick it). */
  hint?: ReactNode | undefined;
}

/** A native checkbox with its label and an optional hint, both clickable. */
export function Checkbox({ label, hint, id, ...rest }: CheckboxProps) {
  const generated = useId();
  const controlId = id ?? generated;
  const hintId = `${controlId}-hint`;
  return (
    <div className={styles["field"]}>
      <input
        {...rest}
        id={controlId}
        type="checkbox"
        className={styles["box"]}
        aria-describedby={hint === undefined ? undefined : hintId}
      />
      <span className={styles["text"]}>
        <label htmlFor={controlId} className={styles["label"]}>
          {label}
        </label>
        {hint === undefined ? null : (
          <span id={hintId} className={styles["hint"]}>
            {hint}
          </span>
        )}
      </span>
    </div>
  );
}
