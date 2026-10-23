import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Chip, ChipRow, TextField } from "~/lib/ui";

import { dayLabel, dueChoices } from "../model";

export interface DueChipsProps {
  /** The group's name for assistive tech ("Due"). */
  label: string;
  /** The chosen day ("2026-10-26"), or none. */
  value: string | undefined;
  /** A chip pressed again clears the day. */
  onChange: (day: string | undefined) => void;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
  /** Prefix for element ids, so two sets on a page stay apart. */
  idPrefix: string;
}

/**
 * The due chips of the quick add, the reader's form and a task's detail:
 * Today, Tomorrow, the next weekday and Pick a date. Never a date typed in
 * words: a chip is faster and cannot guess wrong.
 */
export function DueChips({ label, value, onChange, now, idPrefix }: DueChipsProps) {
  const { t: typed, i18n } = useTranslation("tasks");
  const t = typed as unknown as (key: string) => string;
  const choices = dueChoices(now ?? new Date());
  const chosen = choices.find((choice) => choice.day === value);
  // A day that is none of the chips shows as the picked date.
  const picked = value !== undefined && chosen === undefined;
  const [picking, setPicking] = useState(false);
  return (
    <div className="flex flex-col gap-2">
      <ChipRow label={label}>
        {choices.map((choice) => (
          <Chip
            key={choice.id}
            pressed={choice.day === value}
            onClick={() => {
              setPicking(false);
              onChange(choice.day === value ? undefined : choice.day);
            }}
          >
            {choice.id === "weekday" ? dayLabel(choice.day, i18n.language) : t(`add.${choice.id}`)}
          </Chip>
        ))}
        <Chip
          pressed={picked || picking}
          onClick={() => {
            setPicking((open) => !open);
          }}
        >
          {picked ? dayLabel(value, i18n.language) : t("add.pick")}
        </Chip>
      </ChipRow>
      {picking || picked ? (
        <TextField
          id={`${idPrefix}-date`}
          type="date"
          label={t("add.date")}
          hideLabel
          value={value ?? ""}
          onChange={(event) => {
            onChange(event.target.value === "" ? undefined : event.target.value);
          }}
        />
      ) : null}
    </div>
  );
}
