import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  Checkbox,
  DatePicker,
  Dialog,
  Select,
  TextField,
  TimePicker,
  type SelectOption,
} from "~/lib/ui";

import type { EventKind, EventPatch, EventView, NewEvent } from "../../api/types";
import { MEMBER_KINDS } from "../../api/types";
import { addDays, atWallClock, dayOf, timeField, type Ymd } from "../../model";

/** What the form starts with: an event to edit, or a new one's day, kind and subject. */
export interface EventDraft {
  event?: EventView | undefined;
  day: Ymd;
  kind?: EventKind | undefined;
  /** The email it is made from: shown as a hint. */
  subject?: string | undefined;
}

export interface EventDialogProps {
  draft: EventDraft;
  /** The member's time zone: times are written in it. */
  zone: string;
  saving: boolean;
  failed: boolean;
  onCreate: (event: NewEvent) => void;
  onEdit: (event: EventView, changes: EventPatch) => void;
  onClose: () => void;
}

const LAST_MINUTE = 23 * 60 + 59;
const minutesOf = (time: string) => Number(time.slice(0, 2)) * 60 + Number(time.slice(3, 5));
const timeOf = (minutes: number) =>
  `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;

/**
 * New event, and Edit: title, kind, day, times, place. The day and times come
 * from the date and time pickers; the end is never before the start, and
 * moving the start moves an end it would pass, keeping the event's length.
 */
export function EventDialog({
  draft,
  zone,
  saving,
  failed,
  onCreate,
  onEdit,
  onClose,
}: EventDialogProps) {
  const { t } = useTranslation("schedule");
  const { t: tKinds } = useTranslation("schedule", { keyPrefix: "kind" });
  const editing = draft.event;
  const [title, setTitle] = useState(editing?.title ?? "");
  const [kind, setKind] = useState<EventKind>(editing?.kind ?? draft.kind ?? "none");
  const [day, setDay] = useState(editing === undefined ? draft.day : dayOf(editing.starts));
  const [allDay, setAllDay] = useState(editing?.all_day ?? false);
  const [start, setStart] = useState(
    editing === undefined || editing.all_day ? "09:00" : timeField(editing.starts),
  );
  const [end, setEnd] = useState(
    editing === undefined || editing.all_day ? "10:00" : timeField(editing.ends),
  );
  const [place, setPlace] = useState(editing?.place ?? "");

  const kindOptions: SelectOption<EventKind>[] = [
    ...MEMBER_KINDS,
    ...(kind === "hold" ? (["hold"] as const) : []),
  ].map((each) => ({ value: each, label: tKinds(each) }));
  const valid = title.trim() !== "" && day !== "" && (allDay || (start !== "" && end !== ""));
  const changeStart = (next: string) => {
    setStart(next);
    if (end !== "" && end <= next) {
      const length = start === "" ? 0 : minutesOf(end) - minutesOf(start);
      setEnd(timeOf(Math.min(LAST_MINUTE, minutesOf(next) + (length > 0 ? length : 60))));
    }
  };
  const save = () => {
    if (!valid) {
      return;
    }
    const starts = allDay ? day : atWallClock(zone, day, start);
    // Ending before it starts means it ends the next day.
    const endDay = !allDay && end <= start ? addDays(day, 1) : day;
    const ends = allDay ? addDays(day, 1) : atWallClock(zone, endDay, end);
    if (editing === undefined) {
      onCreate({
        title: title.trim(),
        kind,
        starts,
        ends,
        ...(place.trim() === "" ? {} : { place: place.trim() }),
      });
    } else {
      onEdit(editing, { title: title.trim(), kind, starts, ends, place: place.trim() });
    }
  };

  return (
    <Dialog
      open
      onClose={onClose}
      title={editing === undefined ? t("form.newTitle") : t("form.editTitle")}
      description={
        draft.subject === undefined ? undefined : t("form.fromEmail", { subject: draft.subject })
      }
      visor={null}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("form.cancel")}
          </Button>
          <Button variant="primary" disabled={!valid || saving} onClick={save}>
            {editing === undefined ? t("form.add") : t("form.save")}
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          save();
        }}
      >
        <TextField
          label={t("form.name")}
          value={title}
          error={failed ? t("form.error") : undefined}
          onChange={(event) => {
            setTitle(event.target.value);
          }}
        />
        <Select label={t("form.kind")} value={kind} options={kindOptions} onChange={setKind} />
        <DatePicker
          label={t("form.day")}
          value={day === "" ? undefined : day}
          onChange={(next) => {
            if (next !== undefined) {
              setDay(next);
            }
          }}
        />
        <Checkbox
          label={t("form.allDay")}
          checked={allDay}
          onChange={(event) => {
            setAllDay(event.target.checked);
          }}
        />
        {allDay ? null : (
          <div className="flex flex-col gap-3 sm:flex-row">
            <div className="flex-1">
              <TimePicker
                label={t("form.starts")}
                value={start === "" ? undefined : start}
                onChange={changeStart}
              />
            </div>
            <div className="flex-1">
              <TimePicker
                label={t("form.ends")}
                value={end === "" ? undefined : end}
                min={start === "" ? undefined : start}
                onChange={setEnd}
              />
            </div>
          </div>
        )}
        <TextField
          label={t("form.place")}
          value={place}
          onChange={(event) => {
            setPlace(event.target.value);
          }}
        />
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
