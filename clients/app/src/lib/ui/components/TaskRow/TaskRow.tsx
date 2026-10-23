import { Icon } from "../Icon/Icon";
import { Tag } from "../Tag/Tag";

import styles from "./TaskRow.module.css";

/** A fact about the task, drawn as a chip: its due date, its plan. */
export interface TaskRowChip {
  text: string;
  /** `danger` for a date that has passed. */
  tone?: "neutral" | "danger" | undefined;
}

export interface TaskRowProps {
  title: string;
  /** "Do" or "Pay", as the tag says it. */
  kind: string;
  /** Where it came from: "From Ana Ribeiro · email", "You wrote this". */
  meta: string;
  chips?: readonly TaskRowChip[] | undefined;
  /** The round check's name: "Mark done: Send the signed lease". */
  checkLabel?: string | undefined;
  /** The keyboard cursor (J/K) is on it. */
  selected?: boolean | undefined;
  /** It is open in the detail beside the list. */
  current?: boolean | undefined;
  /** Element id, so the list can scroll the cursor into view. */
  id?: string | undefined;
  /** Open it (a click, Enter). */
  onOpen: () => void;
  /** Mark it done (paid, for a bill); without it the row has no check (a task someone else holds). */
  onCheck?: (() => void) | undefined;
}

/**
 * A row of the Tasks list: a round check, the Do or Pay tag, the title on one
 * line, where it came from, and chips for its date and plan. The check and
 * the row are two buttons side by side, never one inside the other.
 */
export function TaskRow({
  title,
  kind,
  meta,
  chips = [],
  checkLabel,
  selected = false,
  current = false,
  id,
  onOpen,
  onCheck,
}: TaskRowProps) {
  return (
    <div id={id} className={styles["row"]} data-selected={selected || undefined}>
      {onCheck === undefined ? (
        <span className={styles["check"]} data-absent="" aria-hidden="true" />
      ) : (
        <button type="button" className={styles["check"]} aria-label={checkLabel} onClick={onCheck}>
          <Icon name="check" />
        </button>
      )}
      <button
        type="button"
        className={styles["open"]}
        aria-current={current ? "true" : undefined}
        onClick={onOpen}
      >
        <span className={styles["kind"]}>
          <Tag>{kind}</Tag>
        </span>
        <span className={styles["title"]}>{title}</span>
        <span className={styles["meta"]}>{meta}</span>
        {chips.length === 0 ? null : (
          <span className={styles["chips"]}>
            {chips.map((chip) => (
              <Tag key={chip.text} tone={chip.tone ?? "neutral"}>
                {chip.text}
              </Tag>
            ))}
          </span>
        )}
      </button>
    </div>
  );
}
