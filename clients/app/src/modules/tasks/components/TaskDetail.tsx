import { useNavigate } from "@tanstack/react-router";
import { useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { taskRef, useScoutChat } from "~/lib/scout-chat";
import {
  Avatar,
  Button,
  ButtonLink,
  Chip,
  ChipRow,
  DatePicker,
  Heading,
  PaneHeader,
  ReaderBody,
  Spacer,
  Tag,
  Text,
  TextField,
} from "~/lib/ui";

import type { TaskView } from "../api/queries";
import { dayLabel, isLater, isoDay, laterChoices, laterOnDay, whenLabel } from "../model";
import type { TaskSource } from "../source";

import { DueChips } from "./DueChips";
import { HandOff } from "./HandOff";
import type { useTaskActions } from "./use-task-actions";

export interface TaskDetailProps {
  workspaceId: string;
  task: TaskView;
  /** The caller. */
  me: string | undefined;
  /** Where it came from, when it came from a conversation. */
  source: TaskSource | undefined;
  actions: ReturnType<typeof useTaskActions>;
  /** Later's choices are showing (the `s` key opens them). */
  laterOpen: boolean;
  onLaterOpen: (open: boolean) => void;
  /** Below `lg` the detail replaces the list: this goes back. */
  onBack: () => void;
  now?: Date | undefined;
}

/** A task opened: its title, date, plan, where it came from, hand-off and Done. */
export function TaskDetail({
  workspaceId,
  task,
  me,
  source,
  actions,
  laterOpen,
  onLaterOpen,
  onBack,
  now,
}: TaskDetailProps) {
  const { t: typed, i18n } = useTranslation("tasks");
  const t = typed as unknown as (key: string, options?: Record<string, unknown>) => string;
  const navigate = useNavigate();
  const [title, setTitle] = useState(task.title);
  const [changingDue, setChangingDue] = useState(false);
  const [pickingDay, setPickingDay] = useState(false);
  const [pickingLater, setPickingLater] = useState(false);
  const clock = now ?? new Date();
  // The title is edited in place and saved when the field is left.
  const [shownId, setShownId] = useState(task.id);
  if (shownId !== task.id) {
    setShownId(task.id);
    setTitle(task.title);
    setChangingDue(false);
    setPickingDay(false);
    setPickingLater(false);
  }
  const mine = me === undefined || task.assignee === me;
  const pay = task.verb === "pay";
  const saveTitle = () => {
    const next = title.trim();
    if (next === "" || next === task.title) {
      setTitle(task.title);
      return;
    }
    actions.rename(task, next);
  };
  const later = isLater(task, clock);
  // Ask Scout (`.`): the chat about this task; the focus comes back to
  // what it was asked from. Asking changes nothing, so a giver asks too.
  const scout = useScoutChat();
  const askScout = () => {
    const active = document.activeElement;
    scout?.(taskRef(task.id), {
      from: active instanceof HTMLElement && active !== document.body ? active : null,
    });
  };
  useShortcut("scout.ask", askScout, { enabled: scout !== undefined });

  const planChip =
    task.plan === "date" && task.plan_on !== null && task.plan_on !== undefined
      ? t("plan.planned", { date: dayLabel(task.plan_on, i18n.language) })
      : later && task.later_until !== null && task.later_until !== undefined
        ? t("plan.later", { date: whenLabel(task.later_until, i18n.language) })
        : undefined;

  const openEmail = (event: MouseEvent<HTMLElement>) => {
    if (source === undefined || event.metaKey || event.ctrlKey || event.shiftKey) {
      return;
    }
    event.preventDefault();
    void navigate({ to: source.href });
  };

  return (
    <>
      <PaneHeader>
        <span className="flex lg:hidden">
          <Button variant="ghost" icon="chevron-left" shortcut="overlay.close" onClick={onBack}>
            {t("detail.back")}
          </Button>
        </span>
        <Spacer />
        {scout === undefined ? null : (
          <Button variant="ghost" shortcut="scout.ask" onClick={askScout}>
            {t("detail.askScout")}
          </Button>
        )}
      </PaneHeader>
      <ReaderBody>
        <div className="flex flex-col gap-3">
          <TextField
            label={t("detail.titleLabel")}
            hideLabel
            size="title"
            value={title}
            disabled={!mine}
            onChange={(event) => {
              setTitle(event.target.value);
            }}
            onBlur={saveTitle}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              }
            }}
          />
          <div className="flex flex-wrap items-center gap-2">
            <Tag>{t(pay ? "add.pay" : "add.do")}</Tag>
            <Text tone="secondary">
              {task.due_on === null || task.due_on === undefined
                ? t("detail.noDate")
                : t("detail.dueOn", { date: dayLabel(task.due_on, i18n.language) })}
            </Text>
            {mine ? (
              <Button
                variant="link"
                onClick={() => {
                  setChangingDue((open) => !open);
                }}
              >
                {t("detail.change")}
              </Button>
            ) : null}
          </div>
          {changingDue && mine ? (
            <div className="flex flex-col gap-2">
              <DueChips
                label={t("add.due")}
                value={task.due_on ?? undefined}
                now={now}
                onChange={(day) => {
                  actions.setDue(task, day ?? null);
                  setChangingDue(false);
                }}
              />
              {task.due_on === null || task.due_on === undefined ? null : (
                <div>
                  <Button
                    variant="ghost"
                    onClick={() => {
                      actions.setDue(task, null);
                      setChangingDue(false);
                    }}
                  >
                    {t("detail.noDate")}
                  </Button>
                </div>
              )}
            </div>
          ) : null}
        </div>

        {mine ? (
          <section className="flex flex-col gap-2">
            <Heading level={3} size="section">
              {t("detail.when")}
            </Heading>
            {planChip === undefined ? null : (
              <div>
                <Tag>{planChip}</Tag>
              </div>
            )}
            <div className="flex flex-wrap gap-2">
              <Button
                aria-pressed={pickingDay}
                onClick={() => {
                  setPickingDay((open) => !open);
                  setPickingLater(false);
                  onLaterOpen(false);
                }}
              >
                {t("detail.pickDay")}
              </Button>
              <Button
                shortcut="item.snooze"
                aria-pressed={laterOpen}
                onClick={() => {
                  setPickingDay(false);
                  onLaterOpen(!laterOpen);
                }}
              >
                {t("detail.later")}
              </Button>
            </div>
            {pickingDay ? (
              <DatePicker
                inline
                label={t("add.date")}
                value={task.plan_on ?? undefined}
                min={isoDay(clock)}
                now={now}
                onChange={(day) => {
                  if (day !== undefined) {
                    actions.planDay(task, day);
                  }
                  setPickingDay(false);
                }}
              />
            ) : null}
            {laterOpen ? (
              <div className="flex flex-col gap-2">
                <ChipRow label={t("detail.laterChoices")}>
                  {laterChoices(clock).map((choice) => (
                    <Chip
                      key={choice.id}
                      onClick={() => {
                        actions.later(task, choice.until);
                        onLaterOpen(false);
                      }}
                    >
                      {t(`detail.${choice.id}`)}
                    </Chip>
                  ))}
                  <Chip
                    pressed={pickingLater}
                    onClick={() => {
                      setPickingLater((open) => !open);
                    }}
                  >
                    {t("add.pick")}
                  </Chip>
                </ChipRow>
                {pickingLater ? (
                  <DatePicker
                    inline
                    label={t("add.date")}
                    value={undefined}
                    min={isoDay(clock)}
                    now={now}
                    onChange={(day) => {
                      if (day !== undefined) {
                        actions.later(task, laterOnDay(day));
                      }
                      setPickingLater(false);
                      onLaterOpen(false);
                    }}
                  />
                ) : null}
              </div>
            ) : null}
          </section>
        ) : null}

        {source === undefined ? null : (
          <section className="flex flex-col gap-2">
            <Heading level={3} size="section">
              {t("detail.source")}
            </Heading>
            {task.origin === "ask" ? (
              <Text tone="secondary" size="sm">
                {t("detail.foundByScout")}
              </Text>
            ) : null}
            <div className="flex items-center gap-2">
              <Avatar name={source.name} />
              <div className="flex min-w-0 flex-col">
                <Text weight="medium">{source.name}</Text>
                <Text tone="secondary" size="sm">
                  {t("detail.sourceLine", {
                    subject: source.subject,
                    date: whenLabel(source.date, i18n.language),
                  })}
                </Text>
              </div>
            </div>
            <div>
              <ButtonLink variant="link" href={source.href} onClick={openEmail}>
                {t("detail.openEmail")}
              </ButtonLink>
            </div>
          </section>
        )}

        <HandOff
          workspaceId={workspaceId}
          task={task}
          me={me}
          onHandOff={(member) => {
            actions.handOff(task, member);
          }}
          onTakeBack={() => {
            if (me !== undefined) {
              actions.takeBack(task, me);
            }
          }}
        />

        {mine ? (
          <div className="flex flex-col gap-2">
            <div className="flex flex-wrap gap-2">
              <Button
                variant="secondary"
                shortcut="item.done"
                onClick={() => {
                  actions.done(task);
                }}
              >
                {t(pay ? "detail.paid" : "detail.done")}
              </Button>
              <Button
                variant="secondary"
                shortcut="conversations.resolve_need"
                onClick={() => {
                  actions.notNeeded(task);
                }}
              >
                {t("detail.notNeeded")}
              </Button>
            </div>
            {pay ? (
              <Text tone="tertiary" size="sm">
                {t("detail.payNote")}
              </Text>
            ) : null}
          </div>
        ) : null}
      </ReaderBody>
    </>
  );
}
