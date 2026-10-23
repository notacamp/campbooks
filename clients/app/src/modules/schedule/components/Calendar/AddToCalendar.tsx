import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button, CalendarDot, Checkbox, Popover, PopoverSection, type ButtonSize } from "~/lib/ui";

import type { CalendarView } from "../../api/types";

export interface AddToCalendarProps {
  /** The provider calendars that can be written. */
  calendars: readonly CalendarView[];
  /** The one the member added to last: first in the list. */
  lastUsed?: string | null | undefined;
  /** The kind's name for "Always add flights to this calendar". */
  kindName: string;
  onPick: (calendar: CalendarView, always: boolean) => void;
  /** Bind A to open the list (only where one such button is on screen). */
  bindKey?: boolean | undefined;
  size?: ButtonSize | undefined;
}

/** "Add to my calendar": the member's writable calendars, the last used first. */
export function AddToCalendar({
  calendars,
  lastUsed,
  kindName,
  onPick,
  bindKey = false,
  size = "md",
}: AddToCalendarProps) {
  const { t } = useTranslation("schedule");
  const [open, setOpen] = useState(false);
  const [always, setAlways] = useState(false);
  useShortcut(
    "schedule.push_event",
    () => {
      setOpen(true);
    },
    { enabled: bindKey && calendars.length > 0 },
  );
  if (calendars.length === 0) {
    return null;
  }
  const ordered = calendars.toSorted(
    (a, b) => Number(b.id === lastUsed) - Number(a.id === lastUsed),
  );
  return (
    <Popover
      label={t("sheet.pick")}
      placement="bottom-end"
      open={open}
      onOpenChange={setOpen}
      renderTrigger={(trigger) => (
        <Button
          {...trigger}
          variant="primary"
          size={size}
          icon="plus"
          shortcut={bindKey ? "schedule.push_event" : undefined}
        >
          {t("sheet.addToMine")}
        </Button>
      )}
    >
      {(close) => (
        <PopoverSection title={t("sheet.pick")}>
          {ordered.map((calendar) => (
            <Button
              key={calendar.id}
              variant="ghost"
              leading={<CalendarDot color={calendar.color} />}
              onClick={() => {
                close();
                onPick(calendar, always);
              }}
            >
              {calendar.name}
            </Button>
          ))}
          <Checkbox
            label={t("sheet.always", { kind: kindName })}
            checked={always}
            onChange={(event) => {
              setAlways(event.target.checked);
            }}
          />
        </PopoverSection>
      )}
    </Popover>
  );
}
