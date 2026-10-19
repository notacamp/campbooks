import { useId, type ReactNode } from "react";

import { Icon } from "../Icon/Icon";
import { Tag } from "../Tag/Tag";

import styles from "./ChoiceCards.module.css";

export interface ChoiceCardOption<T extends string> {
  value: T;
  title: string;
  description?: string | undefined;
  /** A short label after the title ("Scout's guess"). */
  tag?: string | undefined;
  /** Drawn before the text (a workspace mark, an icon). */
  leading?: ReactNode | undefined;
}

export interface ChoiceCardsProps<T extends string> {
  /** The question, as the group's legend. */
  label: string;
  /** Keep the legend for screen readers only, when a heading already asks. */
  hideLabel?: boolean | undefined;
  options: readonly ChoiceCardOption<T>[];
  value: T | null;
  onChange: (value: T) => void;
  /** Radios share a name; defaults to a generated one. */
  name?: string | undefined;
}

/** The ids a card is described by: its tag, then its sentence. */
function describedBy<T extends string>(
  id: string,
  option: ChoiceCardOption<T>,
): string | undefined {
  const ids = [
    ...(option.tag === undefined ? [] : [`${id}-tag`]),
    ...(option.description === undefined ? [] : [`${id}-description`]),
  ];
  return ids.length === 0 ? undefined : ids.join(" ");
}

/**
 * One choice among a few, each with room for a sentence (Business or
 * Personal). Native radios: arrow keys move, Space picks.
 */
export function ChoiceCards<T extends string>({
  label,
  hideLabel = false,
  options,
  value,
  onChange,
  name,
}: ChoiceCardsProps<T>) {
  const generated = useId();
  const groupName = name ?? generated;
  return (
    <fieldset className={styles["group"]}>
      <legend className={styles["legend"]} data-hidden={hideLabel || undefined}>
        {label}
      </legend>
      {options.map((option) => {
        const id = `${groupName}-${option.value}`;
        const checked = option.value === value;
        return (
          <label key={option.value} className={styles["card"]} htmlFor={id}>
            <input
              id={id}
              className={styles["radio"]}
              type="radio"
              name={groupName}
              value={option.value}
              checked={checked}
              aria-labelledby={`${id}-title`}
              aria-describedby={describedBy(id, option)}
              onChange={() => {
                onChange(option.value);
              }}
            />
            {option.leading}
            <span className={styles["text"]}>
              <span className={styles["heading"]}>
                <span id={`${id}-title`} className={styles["title"]}>
                  {option.title}
                </span>
                {option.tag === undefined ? null : (
                  <span id={`${id}-tag`}>
                    <Tag>{option.tag}</Tag>
                  </span>
                )}
              </span>
              {option.description === undefined ? null : (
                <span id={`${id}-description`} className={styles["description"]}>
                  {option.description}
                </span>
              )}
            </span>
            <span className={styles["check"]} aria-hidden="true">
              <Icon name="check" />
            </span>
          </label>
        );
      })}
    </fieldset>
  );
}
