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
  SheetTitle,
  SheetTop,
  SourceLink,
} from "~/lib/ui";

import type { TaskSource, TaskView } from "~/modules/tasks";

import type { TaskRow } from "../../due";
import { dueTitle, dueWhen, type T } from "../../labels";
import { dayLong, dayOf } from "../../model";
import { useInAppLink } from "./links";

export interface DuePaneProps {
  workspaceId: string;
  /** The row that was opened: it says whether the Due or the Planned chip shows. */
  row: TaskRow;
  today: string;
  /** The email the task came from, when it came from one and the email is known. */
  source: TaskSource | undefined;
  /** The tasks module's Done (Paid), with its own Undo. */
  done: (task: TaskView) => void;
  onClose: () => void;
}

/**
 * A task on this day, due or planned. It lives in the member's tasks, so the
 * sheet has no Remove: Done or Paid (E), Open task (↵).
 */
export function DuePane({ workspaceId, row, today, source, done, onClose }: DuePaneProps) {
  const { t: typed, i18n } = useTranslation("schedule");
  const t: T = typed;
  const locale = i18n.language;
  const navigate = useNavigate();
  const link = useInAppLink();
  const { task } = row;
  const paying = task.verb === "pay";
  const open = () => {
    void navigate({ to: `/w/${workspaceId}/tasks/${task.id}` });
  };
  const finish = () => {
    done(task);
  };
  useShortcut("overlay.close", onClose);
  useShortcut("item.done", finish);
  useShortcut("list.open", open);

  const planned =
    task.plan === "date" && task.plan_on !== null && task.plan_on !== undefined
      ? task.plan_on
      : undefined;
  const due = task.due_on ?? undefined;
  const late = row.kind === "due" && row.late > 0;
  const received = source === undefined ? "" : dayLong(locale, dayOf(source.date));
  return (
    <EventSheet label={dueTitle(t, row)}>
      <SheetTop>
        <KindChip kind="due">{t(row.kind === "due" ? "kind.due" : "kind.planned")}</KindChip>
        <IconButton
          icon="close"
          label={t("sheet.close")}
          shortcut="overlay.close"
          onClick={onClose}
        />
      </SheetTop>
      <SheetTitle>{dueTitle(t, row)}</SheetTitle>
      <SheetFacts>
        {planned === undefined ? null : (
          <SheetFact icon="event" main={t("due.plannedFact", { day: dayLong(locale, planned) })} />
        )}
        {due === undefined ? null : (
          <SheetFact
            icon="clock"
            main={t(late ? "due.wasDue" : "due.dueOn", { day: dayLong(locale, due) })}
          >
            {row.kind === "due" ? dueWhen(t, locale, row, today) : undefined}
          </SheetFact>
        )}
      </SheetFacts>
      <SheetNotice>{t("due.note")}</SheetNotice>
      {source === undefined ? null : (
        <SourceLink
          label={t("sheet.source.label")}
          subject={source.subject}
          meta={
            <>
              {t("sheet.source.meta", { sender: source.name, when: received })}
              {" · "}
              {t("sheet.source.open")}
            </>
          }
          {...link(source.href)}
        />
      )}
      <SheetActions>
        <Button size="lg" variant="primary" icon="check" shortcut="item.done" onClick={finish}>
          {paying ? t("due.paid") : t("due.done")}
        </Button>
        <Button size="lg" variant="ghost" shortcut="list.open" onClick={open}>
          {t("due.open")}
        </Button>
      </SheetActions>
    </EventSheet>
  );
}
