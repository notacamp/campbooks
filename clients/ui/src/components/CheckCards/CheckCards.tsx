import { useId, type ReactNode } from "react";

import { Icon } from "../Icon/Icon";

import styles from "./CheckCards.module.css";

export interface CheckCardOption<T extends string> {
  value: T;
  title: string;
  description?: string | undefined;
  /** Drawn before the text (an avatar, an icon). */
  leading?: ReactNode | undefined;
  /** A short fact at the end ("41 emails"). */
  meta?: string | undefined;
}

export interface CheckCardsProps<T extends string> {
  /** The question, as the group's legend. */
  label: string;
  /** Keep the legend for screen readers only, when a heading already asks. */
  hideLabel?: boolean | undefined;
  options: readonly CheckCardOption<T>[];
  /** The values that are on. */
  value: readonly T[];
  onChange: (value: T[]) => void;
  /** Checkboxes share a name; defaults to a generated one. */
  name?: string | undefined;
}

/**
 * Any number of choices among a few, each with room for a sentence (people
 * to always see). Native checkboxes: Tab moves, Space toggles. The twin of
 * {@link ChoiceCards}, which picks exactly one.
 */
export function CheckCards<T extends string>({
  label,
  hideLabel = false,
  options,
  value,
  onChange,
  name,
}: CheckCardsProps<T>) {
  const generated = useId();
  const groupName = name ?? generated;
  return (
    <fieldset className={styles["group"]}>
      <legend className={styles["legend"]} data-hidden={hideLabel || undefined}>
        {label}
      </legend>
      {options.map((option) => {
        const id = `${groupName}-${option.value}`;
        const checked = value.includes(option.value);
        return (
          <label key={option.value} className={styles["card"]} htmlFor={id}>
            <input
              id={id}
              className={styles["box"]}
              type="checkbox"
              name={groupName}
              value={option.value}
              checked={checked}
              aria-labelledby={`${id}-title`}
              aria-describedby={option.description === undefined ? undefined : `${id}-description`}
              onChange={() => {
                onChange(
                  checked
                    ? value.filter((other) => other !== option.value)
                    : [...value, option.value],
                );
              }}
            />
            {option.leading}
            <span className={styles["text"]}>
              <span id={`${id}-title`} className={styles["title"]}>
                {option.title}
              </span>
              {option.description === undefined ? null : (
                <span id={`${id}-description`} className={styles["description"]}>
                  {option.description}
                </span>
              )}
            </span>
            {option.meta === undefined ? null : (
              <span className={styles["meta"]}>{option.meta}</span>
            )}
            <span className={styles["check"]} aria-hidden="true">
              <Icon name="check" />
            </span>
          </label>
        );
      })}
    </fieldset>
  );
}
