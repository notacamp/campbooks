import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Button,
  EventSheet,
  IconButton,
  KindChip,
  SheetActions,
  SheetFact,
  SheetFacts,
  SheetNotice,
  SheetQuote,
  SheetSection,
  SheetTitle,
  SheetTop,
  SourceLink,
} from "~/lib/ui";

import type { DueAction, DueItem, useDueActions } from "../../due";
import { dueTitle, dueWhen, overdueDays, type T } from "../../labels";
import { dayLong, dayOf } from "../../model";
import { emailPath, useInAppLink } from "./links";

export interface DuePaneProps {
  workspaceId: string;
  item: DueItem;
  today: string;
  act: ReturnType<typeof useDueActions>;
  onClose: () => void;
}

/**
 * A task due on this day. It lives in the member's tasks, so the sheet has
 * no Remove: Paid or Done (E), Hold time (H), Later (S), Open task (↵).
 */
export function DuePane({ workspaceId, item, today, act, onClose }: DuePaneProps) {
  const { t: typed, i18n } = useTranslation("schedule");
  const t = typed as unknown as T;
  const locale = i18n.language;
  const navigate = useNavigate();
  const link = useInAppLink();
  const paying = item.verb === "pay";
  const late = overdueDays(item, today) > 0;
  const run = (action: DueAction) => {
    act.mutate({ item, action });
  };
  const open = () => {
    void navigate({ to: `/w/${workspaceId}/tasks/${item.id}` });
  };
  useShortcut("overlay.close", onClose);
  useShortcut("item.done", () => {
    run("done");
  });
  useShortcut("item.hold", () => {
    run("hold");
  });
  useShortcut("item.snooze", () => {
    run("later");
  });
  useShortcut("list.open", open);

  const source = item.source ?? undefined;
  const received =
    source?.sentAt === null || source?.sentAt === undefined
      ? ""
      : dayLong(locale, dayOf(source.sentAt));
  const day = dayLong(locale, dayOf(item.due));
  return (
    <EventSheet label={dueTitle(t, locale, item)}>
      <SheetTop>
        <KindChip kind="due">{t("kind.due")}</KindChip>
        <IconButton
          icon="close"
          label={t("sheet.close")}
          shortcut="overlay.close"
          onClick={onClose}
        />
      </SheetTop>
      <SheetTitle>{dueTitle(t, locale, item)}</SheetTitle>
      <SheetFacts>
        <SheetFact icon="clock" main={late ? t("due.wasDue", { day }) : t("due.dueOn", { day })}>
          {dueWhen(t, locale, item, today)}
        </SheetFact>
        <SheetFact icon="event" main={t("due.calendar")}>
          {paying ? t("due.markPaid") : t("due.markDone")}
        </SheetFact>
      </SheetFacts>
      <SheetNotice>{t("due.note")}</SheetNotice>
      {act.isError ? <SheetNotice>{t("due.failed")}</SheetNotice> : null}
      {source === undefined ? null : (
        <SourceLink
          label={t("sheet.source.label")}
          subject={source.subject}
          meta={
            <>
              {t("sheet.source.meta", { sender: source.sender, when: received })}
              {" · "}
              {t("sheet.source.open")}
            </>
          }
          {...link(emailPath(workspaceId, source.stream, source.conversationId))}
        />
      )}
      {source?.quote === undefined || source.quote === null ? null : (
        <SheetSection label={t("sheet.how.label")}>
          <SheetQuote>{source.quote}</SheetQuote>
        </SheetSection>
      )}
      <SheetActions>
        <Button
          size="lg"
          variant="primary"
          icon="check"
          shortcut="item.done"
          onClick={() => {
            run("done");
          }}
        >
          {paying ? t("due.paid") : t("due.done")}
        </Button>
        <Button
          size="lg"
          shortcut="item.hold"
          onClick={() => {
            run("hold");
          }}
        >
          {t("due.hold")}
        </Button>
        <Button
          size="lg"
          shortcut="item.snooze"
          onClick={() => {
            run("later");
          }}
        >
          {t("due.later")}
        </Button>
        <Button size="lg" variant="ghost" shortcut="list.open" onClick={open}>
          {t("due.open")}
        </Button>
      </SheetActions>
    </EventSheet>
  );
}
