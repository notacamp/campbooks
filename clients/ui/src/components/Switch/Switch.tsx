import type { ComponentPropsWithRef } from "react";

import { cx } from "../../internal/cx";

import styles from "./Switch.module.css";

export interface SwitchProps extends Omit<
  ComponentPropsWithRef<"button">,
  "children" | "role" | "aria-checked" | "aria-label" | "onChange"
> {
  /** What it turns on or off, for screen readers ("Scout adds Flights"). */
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

/** An on/off switch for a setting that applies at once. */
export function Switch({
  label,
  checked,
  onChange,
  className,
  type = "button",
  ...rest
}: SwitchProps) {
  return (
    <button
      {...rest}
      type={type}
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={cx(styles["switch"], className)}
      onClick={() => {
        onChange(!checked);
      }}
    />
  );
}
