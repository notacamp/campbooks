import { useId, type ComponentPropsWithRef, type ReactNode } from "react";

import { cx } from "../../internal/cx";
import { Icon } from "../Icon/Icon";

import styles from "./Field.module.css";

interface FieldFrameProps {
  /** The visible label (always the accessible name). */
  label: string;
  /** Keep the label for screen readers only, when the context already says it. */
  hideLabel?: boolean | undefined;
  /** Help under the control. */
  hint?: ReactNode | undefined;
  /** What is wrong with the value; marks the control invalid. */
  error?: ReactNode | undefined;
  /** md follows density; lg is the 44px touch size, for one-question screens. */
  size?: "md" | "lg" | undefined;
}

function useFieldIds(id: string | undefined) {
  const generated = useId();
  const controlId = id ?? generated;
  return { controlId, hintId: `${controlId}-hint`, errorId: `${controlId}-error` };
}

function describedBy(
  hint: ReactNode | undefined,
  error: ReactNode | undefined,
  ids: { hintId: string; errorId: string },
) {
  const parts = [hint === undefined ? null : ids.hintId, error === undefined ? null : ids.errorId];
  const value = parts.filter((part) => part !== null).join(" ");
  return value === "" ? undefined : value;
}

function FieldFrame({
  label,
  hideLabel,
  hint,
  error,
  ids,
  children,
}: Omit<FieldFrameProps, "size"> & {
  ids: { controlId: string; hintId: string; errorId: string };
  children: ReactNode;
}) {
  return (
    <div className={styles["field"]}>
      <label
        className={styles["label"]}
        htmlFor={ids.controlId}
        data-hidden={hideLabel === true || undefined}
      >
        {label}
      </label>
      {children}
      {hint === undefined ? null : (
        <p id={ids.hintId} className={styles["hint"]}>
          {hint}
        </p>
      )}
      {error === undefined ? null : (
        <p id={ids.errorId} className={styles["error"]}>
          <Icon name="warning" />
          <span>{error}</span>
        </p>
      )}
    </div>
  );
}

export type TextFieldProps = FieldFrameProps &
  Omit<ComponentPropsWithRef<"input">, "size" | "children">;

/** A labelled text input with optional help and an error message. */
export function TextField({
  label,
  hideLabel,
  hint,
  error,
  size = "md",
  id,
  className,
  type = "text",
  ...rest
}: TextFieldProps) {
  const ids = useFieldIds(id);
  return (
    <FieldFrame label={label} hideLabel={hideLabel} hint={hint} error={error} ids={ids}>
      <input
        {...rest}
        id={ids.controlId}
        type={type}
        className={cx(styles["control"], className)}
        data-size={size}
        aria-invalid={error === undefined ? undefined : true}
        aria-describedby={describedBy(hint, error, ids)}
      />
    </FieldFrame>
  );
}

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

export type SelectProps<T extends string> = FieldFrameProps &
  Omit<ComponentPropsWithRef<"select">, "size" | "children" | "value" | "onChange"> & {
    options: readonly SelectOption<T>[];
    value: T;
    onChange: (value: T) => void;
  };

/** A labelled native select: the platform's own picker on phones. */
export function Select<T extends string>({
  label,
  hideLabel,
  hint,
  error,
  size = "md",
  id,
  className,
  options,
  value,
  onChange,
  ...rest
}: SelectProps<T>) {
  const ids = useFieldIds(id);
  return (
    <FieldFrame label={label} hideLabel={hideLabel} hint={hint} error={error} ids={ids}>
      <span className={styles["selectWrap"]}>
        <select
          {...rest}
          id={ids.controlId}
          className={cx(styles["control"], styles["select"], className)}
          data-size={size}
          value={value}
          aria-invalid={error === undefined ? undefined : true}
          aria-describedby={describedBy(hint, error, ids)}
          onChange={(event) => {
            const next = options.find((option) => option.value === event.target.value);
            if (next !== undefined) {
              onChange(next.value);
            }
          }}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <Icon name="chevron-down" className={styles["chevron"]} />
      </span>
    </FieldFrame>
  );
}
