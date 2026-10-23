import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Button,
  Chip,
  ChipRow,
  EventSheet,
  IconButton,
  Popover,
  PopoverSection,
  KindChipButton,
  KindIcon,
  SheetActions,
  SheetFact,
  SheetFacts,
  SheetNotice,
  SheetQuote,
  SheetSection,
  SheetTitle,
  SheetTop,
  SourceLink,
  Text,
} from "~/lib/ui";

import {
  quoteFor,
  useMessageRead,
  useSourceMessage,
  type useEventActions,
} from "../../api/queries";
import { MEMBER_KINDS, type CalendarView, type EventView, type Response } from "../../api/types";
import { providerName, whenLong, zoneNote, type T } from "../../labels";
import { canPush, dayLong, dayOf } from "../../model";
import { AddToCalendar } from "./AddToCalendar";
import { emailPath, useInAppLink } from "./links";

export interface EventPaneProps {
  workspaceId: string;
  event: EventView;
  calendars: readonly CalendarView[];
  lastPushed?: string | null | undefined;
  /** The member's time zone. */
  zone: string;
  actions: ReturnType<typeof useEventActions>;
  onClose: () => void;
  onEdit: () => void;
}

const ANSWERS = ["accepted", "tentative", "declined"] as const satisfies readonly Response[];

/**
 * One event, on the right (the whole screen on a phone): its kind (which
 * the member can change), when, where, the calendar; for an event Scout
 * read, the email it came from and the sentence it read the date from;
 * then Add to my calendar, Edit and Remove, and Accept / Maybe / Decline
 * for an invitation.
 */
export function EventPane({
  workspaceId,
  event,
  calendars,
  lastPushed,
  zone,
  actions,
  onClose,
  onEdit,
}: EventPaneProps) {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const navigate = useNavigate();
  const link = useInAppLink();
  const source = useSourceMessage(workspaceId, event);
  const held = event.kind === "hold";
  const model = event.found_by === "model";
  const read = useMessageRead(
    workspaceId,
    event.source_conversation ?? "",
    event.source?.id ?? "",
    { enabled: model && event.source_conversation != null && event.source != null },
  );
  const quote = model ? quoteFor(read.data, event) : undefined;

  const calendar = calendars.find((each) => each.id === event.calendar_id);
  const own = calendar?.source === "campbooks";
  const provider = providerName(calendar);
  const syncedNote =
    provider === undefined ? t("sheet.synced") : t("sheet.syncedWith", { provider });
  const writable = calendars.filter((each) => each.source === "provider" && each.writable);
  const addable = !held && canPush(event, calendars);
  const elsewhere = calendars.find((each) => each.id === lastPushed)?.name;
  const note = zoneNote(t, locale, event, zone);
  const remove = () => {
    actions.remove.mutate(event);
    onClose();
  };
  const openEmail = () => {
    if (source.data !== undefined) {
      void navigate({
        to: emailPath(workspaceId, source.data.stream, source.data.conversationId),
      });
    }
  };

  useShortcut("overlay.close", onClose);
  useShortcut("schedule.update_event", onEdit, { enabled: !held });
  useShortcut("schedule.delete_event", remove);
  useShortcut("schedule.open_source", openEmail, { enabled: source.data !== undefined });
  useShortcut(
    "list.open",
    () => {
      void navigate({ to: `/w/${workspaceId}/tasks` });
    },
    { enabled: held },
  );

  const sourceHref =
    source.data === undefined
      ? undefined
      : emailPath(workspaceId, source.data.stream, source.data.conversationId);
  const received =
    source.data?.sentAt === null || source.data?.sentAt === undefined
      ? ""
      : dayLong(locale, dayOf(source.data.sentAt));

  return (
    <EventSheet label={event.title}>
      <SheetTop>
        <Popover
          label={t("sheet.kindMenu")}
          placement="bottom-end"
          renderTrigger={(trigger) => (
            <KindChipButton
              {...trigger}
              kind={event.kind}
              aria-label={t("sheet.kind", { kind: t(`kind.${event.kind}`) })}
            >
              {t(`kind.${event.kind}`)}
            </KindChipButton>
          )}
        >
          {(close) => (
            <PopoverSection title={t("sheet.kindMenu")}>
              {MEMBER_KINDS.map((kind) => (
                <Button
                  key={kind}
                  variant="ghost"
                  leading={<KindIcon kind={kind} />}
                  onClick={() => {
                    close();
                    if (kind !== event.kind) {
                      actions.edit.mutate({ event, changes: { kind } });
                    }
                  }}
                >
                  {t(`kind.${kind}`)}
                </Button>
              ))}
            </PopoverSection>
          )}
        </Popover>
        <IconButton
          icon="close"
          label={t("sheet.close")}
          shortcut="overlay.close"
          onClick={onClose}
        />
      </SheetTop>
      <SheetTitle>{event.title}</SheetTitle>
      <SheetFacts>
        <SheetFact icon="clock" main={whenLong(t, locale, event)}>
          {note ?? (event.recurring ? t("sheet.series") : undefined)}
        </SheetFact>
        {event.place === undefined || event.place === null || event.place === "" ? null : (
          <SheetFact icon="pin">{event.place}</SheetFact>
        )}
        <SheetFact icon="event" main={calendar?.name}>
          {own ? t("sheet.onlyYou") : syncedNote}
        </SheetFact>
      </SheetFacts>
      {event.notice === "kept_theirs" ? (
        <SheetNotice>{t("notice.keptTheirs", { where: provider ?? calendar?.name ?? "" })}</SheetNotice>
      ) : null}
      {event.notice === "not_saved" ? <SheetNotice>{t("notice.notSaved")}</SheetNotice> : null}
      {event.pending ? (
        <SheetNotice>{t("notice.pending", { where: provider ?? calendar?.name ?? "" })}</SheetNotice>
      ) : null}
      {source.data === undefined ? null : (
        <SourceLink
          label={t("sheet.source.label")}
          subject={source.data.subject}
          meta={
            <>
              {t("sheet.source.meta", { sender: source.data.sender, when: received })}
              {" · "}
              {t("sheet.source.open")}
            </>
          }
          shortcut="schedule.open_source"
          {...link(sourceHref ?? "")}
        />
      )}
      {event.origin === "scout" && event.found_by !== undefined && event.found_by !== null ? (
        <SheetSection label={t("sheet.how.label")}>
          <Text tone="secondary" size="sm">
            {model ? t("sheet.how.model") : t("sheet.how.structured")}
          </Text>
          {quote === undefined ? null : <SheetQuote>{quote}</SheetQuote>}
        </SheetSection>
      ) : null}
      {event.response === undefined || event.response === null ? null : (
        <SheetSection label={t("sheet.rsvp")}>
          <ChipRow label={t("sheet.rsvp")}>
            {ANSWERS.map((answer) => (
              <Chip
                key={answer}
                pressed={event.response === answer}
                onClick={() => {
                  if (event.response !== answer) {
                    actions.respond.mutate({ event, response: answer });
                  }
                }}
              >
                {t(`rsvp.${answer}`)}
              </Chip>
            ))}
          </ChipRow>
        </SheetSection>
      )}
      <SheetActions>
        {held ? (
          <>
            <Button
              size="lg"
              variant="secondary"
              shortcut="list.open"
              onClick={() => {
                void navigate({ to: `/w/${workspaceId}/tasks` });
              }}
            >
              {t("due.open")}
            </Button>
            <Button size="lg" variant="ghost" onClick={remove}>
              {t("sheet.release")}
            </Button>
          </>
        ) : (
          <>
            {addable ? (
              <AddToCalendar
                calendars={writable}
                lastUsed={lastPushed}
                kindName={t(`kind.${event.kind}`).toLowerCase()}
                size="lg"
                bindKey
                onPick={(target) => {
                  actions.addTo.mutate({ event, calendar: target });
                }}
              />
            ) : null}
            {own && event.on_provider ? (
              <Text as="span" tone="secondary" size="sm">
                {elsewhere === undefined
                  ? t("sheet.pushed")
                  : t("sheet.alsoIn", { calendar: elsewhere })}
              </Text>
            ) : null}

            <Button size="lg" shortcut="schedule.update_event" icon="edit" onClick={onEdit}>
              {t("sheet.edit")}
            </Button>
            <Button
              size="lg"
              variant="ghost"
              shortcut="schedule.delete_event"
              icon="trash"
              onClick={remove}
            >
              {t("sheet.remove")}
            </Button>
          </>
        )}
      </SheetActions>
    </EventSheet>
  );
}
