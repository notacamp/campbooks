import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, FoundItem, FoundReading, FoundStrip } from "~/lib/ui";

import {
  useCalendars,
  useEventActions,
  useEvents,
  useMessageRead,
  useRequestRead,
  useScheduleSettings,
} from "../../api/queries";
import { SCOUT_KINDS, type EventKind, type ReadItem, type ScoutKind } from "../../api/types";
import type { T } from "../../labels";
import {
  addDays,
  canPush,
  clock,
  dayLong,
  dayOf,
  isShown,
  minutesOf,
  nowIn,
} from "../../model";
import { AddToCalendar } from "../Calendar/AddToCalendar";
import { EventDialog, type EventDraft } from "../Calendar/EventDialog";

export interface FoundInMessageProps {
  workspaceId: string;
  conversationId: string;
  messageId: string;
  /** The message's subject: what a hand-made event says it was made from. */
  subject: string;
}

const isScoutKind = (value: string | null | undefined): value is ScoutKind =>
  (SCOUT_KINDS as readonly (string | null | undefined)[]).includes(value);

/**
 * "Scout noticed", under a message's header: what was read from it, each item
 * with its kind, title and time and, for an event Scout added, "Added to your
 * calendar" with Undo (or the calendar it was pushed to). While a read is
 * queued it says so; a flagged message nobody has read is read on open, once;
 * where no model may read mail, a flagged booking says "This email has a …"
 * with Add, which opens the event form.
 */
export function FoundInMessage({
  workspaceId,
  conversationId,
  messageId,
  subject,
}: FoundInMessageProps) {
  const { t: typed } = useTranslation("schedule");
  const t: T = typed;
  const read = useMessageRead(workspaceId, conversationId, messageId);
  const request = useRequestRead(workspaceId, conversationId, messageId);
  const settings = useScheduleSettings(workspaceId);
  const actions = useEventActions(workspaceId);
  const [draft, setDraft] = useState<EventDraft | undefined>();
  const asked = useRef(false);
  const data = read.data;
  const wantsRead =
    data !== undefined &&
    data.flagged &&
    data.status === "none" &&
    (data.dated_kind === undefined || data.dated_kind === null);
  const { mutate } = request;
  useEffect(() => {
    // Once per opening: a failure shows "Try again" instead of asking again.
    if (wantsRead && !asked.current) {
      asked.current = true;
      mutate();
    }
  }, [wantsRead, mutate]);

  if (data === undefined) {
    return null;
  }
  const zone = settings.data?.effective_time_zone ?? "UTC";
  const today = nowIn(zone, new Date()).day;
  const dialog =
    draft === undefined ? null : (
      <EventDialog
        draft={draft}
        zone={zone}
        saving={actions.create.isPending}
        failed={actions.create.isError}
        onCreate={(body) => {
          actions.create.mutate(body, {
            onSuccess: () => {
              setDraft(undefined);
            },
          });
        }}
        onEdit={() => {
          setDraft(undefined);
        }}
        onClose={() => {
          actions.create.reset();
          setDraft(undefined);
        }}
      />
    );

  let content: ReactNode;
  if (data.items.length > 0) {
    content = data.items.map((item, index) => (
      <FoundEvent
        key={item.key}
        workspaceId={workspaceId}
        item={item}
        first={index === 0}
        onAdd={() => {
          setDraft({
            day: dayOf(item.starts),
            kind: item.kind,
            subject,
            source: { messageId, conversationId },
          });
        }}
      />
    ));
  } else if (data.status === "queued" || request.isPending || wantsRead) {
    content = <FoundReading>{t("found.reading")}</FoundReading>;
  } else if (data.status === "failed") {
    content = (
      <FoundItem
        kind="none"
        title={t("found.failed")}
        meta=""
        actions={
          <Button
            onClick={() => {
              request.mutate();
            }}
          >
            {t("retry")}
          </Button>
        }
      />
    );
  } else if (isScoutKind(data.dated_kind)) {
    const kind = data.dated_kind;
    content = (
      <FoundItem
        kind={kind}
        title={t(`found.has.${kind}`)}
        meta={t("found.cantRead")}
        actions={
          <Button
            onClick={() => {
              setDraft({ day: today, kind, subject, source: { messageId, conversationId } });
            }}
          >
            {t("found.add")}
          </Button>
        }
      />
    );
  } else {
    return null;
  }

  return (
    <>
      <FoundStrip label={t("found.label")}>{content}</FoundStrip>
      {dialog}
    </>
  );
}

/** One thing Scout read from the message, and where it is now. */
function FoundEvent({
  workspaceId,
  item,
  first,
  onAdd,
}: {
  workspaceId: string;
  item: ReadItem;
  first: boolean;
  onAdd: () => void;
}) {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const day = dayOf(item.starts);
  const nearby = useEvents(workspaceId, { from: day, to: addDays(day, 2) });
  const calendars = useCalendars(workspaceId);
  const settings = useScheduleSettings(workspaceId);
  const actions = useEventActions(workspaceId);
  const [removed, setRemoved] = useState<{ token: string; id: string } | undefined>();

  // The event the item became: the one that carries its key.
  const event = nearby.data?.events.find(
    (each) => isShown(each) && each.item_keys.includes(item.key),
  );
  const list = calendars.data ?? [];
  const writable = list.filter((each) => each.source === "provider" && each.writable);
  const when = item.all_day
    ? dayLong(locale, day)
    : `${dayLong(locale, day)}, ${clock(locale, minutesOf(item.starts))}`;
  const place = item.place?.trim();
  const meta = [
    t(`kind.${item.kind}`),
    when,
    place === undefined || place === "" ? undefined : place,
  ]
    .filter((each) => each !== undefined)
    .join(" · ");
  const kind: EventKind = item.kind;

  if (removed !== undefined) {
    return (
      <FoundItem
        kind={item.kind}
        title={item.title}
        meta={meta}
        status={t("found.removed")}
        actions={
          <Button
            variant="ghost"
            onClick={() => {
              void actions.restore(removed.token, removed.id).then(() => {
                setRemoved(undefined);
              });
            }}
          >
            {t("found.putBack")}
          </Button>
        }
      />
    );
  }
  if (event === undefined) {
    return (
      <FoundItem
        kind={item.kind}
        title={item.title}
        meta={meta}
        quote={item.quote ?? undefined}
        actions={<Button onClick={onAdd}>{t("found.add")}</Button>}
      />
    );
  }
  const elsewhere = list.find((each) => each.id === settings.data?.last_push_calendar)?.name;
  return (
    <FoundItem
      kind={item.kind}
      title={item.title}
      meta={meta}
      quote={item.quote ?? undefined}
      status={
        event.on_provider && elsewhere !== undefined
          ? t("found.alsoIn", { calendar: elsewhere })
          : t("found.added")
      }
      actions={
        <>
          {canPush(event, list) ? (
            <AddToCalendar
              calendars={writable}
              lastUsed={settings.data?.last_push_calendar}
              kindName={t(`kind.${kind}`).toLowerCase()}
              bindKey={first}
              onPick={(calendar) => {
                actions.addTo.mutate({ event, calendar });
              }}
            />
          ) : null}
          <Button
            variant="ghost"
            onClick={() => {
              actions.remove.mutate(event, {
                onSuccess: ({ token }) => {
                  setRemoved({ token, id: event.id });
                },
              });
            }}
          >
            {t("found.undo")}
          </Button>
        </>
      }
    />
  );
}
