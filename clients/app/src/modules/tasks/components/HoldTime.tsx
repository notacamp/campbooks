import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Chip, ChipRow, Text } from "~/lib/ui";

import type { TaskView } from "../api/queries";
import {
  DEFAULT_LENGTH,
  HOLD_LENGTHS,
  heldLabel,
  holdSlots,
  holdWindow,
  untilDue,
  type HoldLength,
  type HoldPick,
  type HoldSources,
} from "../hold";

export interface HoldTimeProps {
  workspaceId: string;
  task: TaskView;
  /** The member's free time and writable calendars (the schedule module's, plugged in). */
  sources: HoldSources;
  /** A slot was picked: hold it. */
  onPick: (pick: HoldPick) => void;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * Hold time: the first three free slots of the member's working hours until
 * the task is due (or in the next 7 days), with Length (15, 30 or 60
 * minutes) and Calendar. Picking a slot holds it. The same panel in the
 * task's detail, in Home's dialog and in the calendar's task sheet.
 */
export function HoldTime({
  workspaceId,
  task,
  sources: { useFreeSlots, useWritableCalendars },
  onPick,
  now,
}: HoldTimeProps) {
  const { t: typed, i18n } = useTranslation("tasks");
  const t = typed as unknown as (key: string, options?: Record<string, unknown>) => string;
  // The window is fixed when the panel opens, so its query does not change on every render.
  const [opened] = useState(() => now ?? new Date());
  const [length, setLength] = useState<HoldLength>(DEFAULT_LENGTH);
  const calendars = useWritableCalendars(workspaceId) ?? [];
  const [picked, setPicked] = useState<string | undefined>();
  const calendar = calendars.find((each) => each.id === picked) ?? calendars[0];
  const free = useFreeSlots(workspaceId, holdWindow(task, opened, length));
  const slots = free.slots === undefined ? undefined : holdSlots(free.slots, length, opened);
  const none = untilDue(task, opened) ? t("hold.noneBeforeDue") : t("hold.noneThisWeek");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1">
        <Text tone="secondary" size="sm">
          {t("hold.length")}
        </Text>
        <ChipRow label={t("hold.length")}>
          {HOLD_LENGTHS.map((minutes) => (
            <Chip
              key={minutes}
              pressed={minutes === length}
              onClick={() => {
                setLength(minutes);
              }}
            >
              {t("hold.minutes", { minutes })}
            </Chip>
          ))}
        </ChipRow>
      </div>
      {calendars.length > 1 ? (
        <div className="flex flex-col gap-1">
          <Text tone="secondary" size="sm">
            {t("hold.calendar")}
          </Text>
          <ChipRow label={t("hold.calendar")}>
            {calendars.map((each) => (
              <Chip
                key={each.id}
                pressed={each.id === calendar?.id}
                onClick={() => {
                  setPicked(each.id);
                }}
              >
                {each.campbooks ? t("hold.campbooks") : each.name}
              </Chip>
            ))}
          </ChipRow>
        </div>
      ) : null}
      <div className="flex flex-col gap-1">
        <Text tone="secondary" size="sm">
          {t("hold.slots")}
        </Text>
        {free.failed ? (
          <Text tone="secondary">{t("hold.failed")}</Text>
        ) : slots === undefined ? (
          <Text tone="secondary">{t("hold.looking")}</Text>
        ) : slots.length === 0 ? (
          <Text tone="secondary">{none}</Text>
        ) : (
          <ChipRow label={t("hold.slots")}>
            {slots.map((slot) => (
              <Chip
                key={slot.starts}
                onClick={() => {
                  onPick({ starts: slot.starts, ends: slot.ends, calendarId: calendar?.id });
                }}
              >
                {heldLabel(slot, i18n.language)}
              </Chip>
            ))}
          </ChipRow>
        )}
      </div>
    </div>
  );
}
