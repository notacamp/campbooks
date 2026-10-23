import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { describedBy, FieldFrame, useFieldIds } from "../Field/Field";
import { Icon } from "../Icon/Icon";

import {
  formatTime,
  isTime,
  nearestSlot,
  parseTime,
  timeSlots,
  toMinutes,
  type Time,
} from "./times";
import styles from "./TimePicker.module.css";

export interface TimePickerProps {
  /** The field's name ("Starts"). */
  label: string;
  /** Keep the label for screen readers only. */
  hideLabel?: boolean | undefined;
  /** The time ("09:30", 24-hour), or none. */
  value: string | undefined;
  /** A time was picked from the list, or typed and settled. */
  onChange: (time: string) => void;
  /** Minutes between the listed times. */
  step?: number | undefined;
  /** The earliest time it takes (an end after its start): earlier ones are not listed. */
  min?: string | undefined;
  id?: string | undefined;
  /** Help under the field. */
  hint?: ReactNode | undefined;
  /** What is wrong with the time. */
  error?: ReactNode | undefined;
}

/**
 * A time field: the time in the member's language (9:30 AM, or 09:30), and a
 * list of times every `step` minutes under it (an ARIA combobox). The list
 * opens on focus, a click or ↓; ↑/↓ move, Enter picks, Escape closes. A
 * typed time ("930", "9:30", "21h15", "9pm") is taken on Enter or on leaving
 * the field; anything else goes back to the last time.
 */
export function TimePicker({
  label,
  hideLabel,
  value,
  onChange,
  step = 15,
  min,
  id,
  hint,
  error,
}: TimePickerProps) {
  const { t, i18n } = useTranslation("ui");
  const ids = useFieldIds(id);
  const listId = useId();
  const list = useRef<HTMLDivElement>(null);
  const locale = i18n.language;
  const current = value !== undefined && isTime(value) ? value : undefined;
  const shown = current === undefined ? "" : formatTime(current, locale);
  const [draft, setDraft] = useState(shown);
  const [editing, setEditing] = useState(false);
  const [open, setOpen] = useState(false);
  const slots = timeSlots(step, min !== undefined && isTime(min) ? min : undefined);
  const [active, setActive] = useState(-1);
  // Not typing: the field shows the time as it is.
  const text = editing ? draft : shown;

  const nearest = (time: Time | undefined) => {
    if (time !== undefined) {
      return nearestSlot(slots, time);
    }
    return slots.length > 0 ? 0 : -1;
  };
  const show = (time: Time | undefined = current) => {
    setOpen(true);
    setActive(nearest(time));
  };
  const close = () => {
    setOpen(false);
    setActive(-1);
  };
  // The active time scrolls into view as it moves.
  useEffect(() => {
    if (open && active >= 0) {
      const option: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
        list.current?.querySelector(`[data-index="${String(active)}"]`) ?? null;
      option?.scrollIntoView?.({ block: "nearest" });
    }
  }, [open, active]);

  const pick = (time: Time) => {
    setEditing(false);
    close();
    if (time !== current) {
      onChange(time);
    }
  };
  /** Take what was typed, or go back to the last time. */
  const settle = () => {
    const typed = parseTime(draft);
    setEditing(false);
    close();
    if (
      editing &&
      typed !== undefined &&
      typed !== current &&
      (min === undefined || !isTime(min) || toMinutes(typed) >= toMinutes(min))
    ) {
      onChange(typed);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        show(editing ? parseTime(draft) : current);
        return;
      }
      const by = event.key === "ArrowDown" ? 1 : -1;
      const next = Math.min(slots.length - 1, Math.max(0, active + by));
      const slot = slots[next];
      if (slot !== undefined) {
        // The field follows the list: Enter or leaving takes it.
        setActive(next);
        setEditing(true);
        setDraft(formatTime(slot, locale));
      }
    } else if (event.key === "Enter") {
      // Never the form's submit: the time settles first.
      event.preventDefault();
      settle();
    } else if (event.key === "Escape" && (open || editing)) {
      // Closes the list only, not the dialog or page around it.
      event.preventDefault();
      event.stopPropagation();
      setEditing(false);
      close();
    }
  };

  const activeId = open && active >= 0 ? `${listId}-${String(active)}` : undefined;
  return (
    <FieldFrame label={label} hideLabel={hideLabel} hint={hint} error={error} ids={ids}>
      <div className={styles["box"]}>
        <Icon name="clock" className={styles["icon"]} />
        <input
          id={ids.controlId}
          type="text"
          role="combobox"
          className={styles["input"]}
          autoComplete="off"
          spellCheck={false}
          placeholder={t("timePicker.choose")}
          value={text}
          aria-expanded={open}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={activeId}
          aria-invalid={error === undefined ? undefined : true}
          aria-describedby={describedBy(hint, error, ids)}
          onFocus={() => {
            show();
          }}
          onClick={() => {
            if (!open) {
              show();
            }
          }}
          onChange={(event) => {
            const typed = event.target.value;
            setEditing(true);
            setDraft(typed);
            // Typing jumps the list to the nearest time.
            const time = parseTime(typed);
            setOpen(true);
            setActive(time === undefined ? -1 : nearest(time));
          }}
          onBlur={settle}
          onKeyDown={onKeyDown}
        />
        <div
          ref={list}
          id={listId}
          role="listbox"
          aria-label={label}
          className={styles["list"]}
          hidden={!open}
        >
          {slots.map((slot, index) => (
            <div
              key={slot}
              id={`${listId}-${String(index)}`}
              role="option"
              tabIndex={-1}
              data-index={index}
              aria-selected={index === active}
              data-current={slot === current || undefined}
              className={styles["option"]}
              onMouseDown={(event) => {
                // Picked before the field's blur, which would settle what was typed.
                event.preventDefault();
                pick(slot);
              }}
              onMouseMove={() => {
                if (index !== active) {
                  setActive(index);
                }
              }}
            >
              {formatTime(slot, locale)}
            </div>
          ))}
        </div>
      </div>
    </FieldFrame>
  );
}
