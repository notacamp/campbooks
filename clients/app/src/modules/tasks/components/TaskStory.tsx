import { useNavigate } from "@tanstack/react-router";
import { useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  Avatar,
  ButtonLink,
  Button,
  Dot,
  Heading,
  SenderSentence,
  Text,
  type Signal,
} from "~/lib/ui";
import { useMembers } from "~/modules/workspaces";

import { useTaskHistory, type TaskHistoryEntry, type TaskView } from "../api/queries";
import { historyLine, historyStamp, visibleHistory, FOLD } from "../history";
import { clockTime, shortDay, whenLabel } from "../model";
import type { TaskSource } from "../source";

import { memberName } from "./HandOff";

/** The dot of who made a change: a member is plain, Scout and the calendar have a colour. */
const DOT: Record<TaskHistoryEntry["actor"], Signal | undefined> = {
  member: undefined,
  scout: "sort",
  calendar: "do",
};

export interface TaskStoryProps {
  workspaceId: string;
  task: TaskView;
  /** The caller. */
  me: string | undefined;
  /** Where it came from, when it came from a conversation. */
  source: TaskSource | undefined;
  /** The sender's own sentence that asked for it, when the caller may read the email. */
  quote?: string | undefined;
}

/** Why a task is here: who made it and when, where it came from, then what happened to it. */
export function TaskStory({ workspaceId, task, me, source, quote }: TaskStoryProps) {
  const { t: typed, i18n } = useTranslation("tasks");
  const t = typed as unknown as (key: string, options?: Record<string, unknown>) => string;
  const navigate = useNavigate();
  const members = useMembers(workspaceId).data ?? [];
  const history = useTaskHistory(workspaceId, task.id).data ?? [];
  const [expanded, setExpanded] = useState(false);
  const locale = i18n.language;

  const byId = new Map(members.map((member) => [member.user_id, member] as const));
  const named = (id: string) => {
    const member = byId.get(id);
    return member === undefined ? undefined : memberName(member);
  };
  const someone = (id: string | null | undefined) =>
    (id === null || id === undefined ? undefined : named(id)) ?? t("history.teammate");

  const when = {
    date: shortDay(task.created_at, locale),
    time: clockTime(task.created_at, locale),
  };
  const lead =
    task.origin === "own"
      ? task.created_by === me
        ? t("why.you")
        : t("why.member", { name: someone(task.created_by) })
      : t("why.scout");
  const rest = task.origin === "own" ? t("why.madeRest", when) : t("why.scoutRest", when);
  const handedAt = history.findLast((entry) => entry.what === "handed_off")?.at;
  const handedToMe =
    task.handed_by !== null && task.handed_by !== undefined && task.handed_by !== me;

  const openEmail = (event: MouseEvent<HTMLElement>) => {
    if (source === undefined || event.metaKey || event.ctrlKey || event.shiftKey) {
      return;
    }
    event.preventDefault();
    void navigate({ to: source.href });
  };

  const shown = visibleHistory(history, expanded);
  const folded = !expanded && history.length > FOLD;

  return (
    <>
      <section className="flex flex-col gap-2">
        <Heading level={3} size="section">
          {t("detail.why")}
        </Heading>
        <Text>
          <Text as="span" weight="medium">
            {lead}
          </Text>{" "}
          {rest}
        </Text>
        {handedToMe ? (
          <Text tone="secondary">
            {handedAt === undefined
              ? t("why.handedNoDate", { name: someone(task.handed_by) })
              : t("why.handed", {
                  name: someone(task.handed_by),
                  date: whenLabel(handedAt, locale),
                })}
          </Text>
        ) : null}
        {source === undefined ? null : (
          <>
            <div className="flex items-center gap-2">
              <Avatar name={source.name} />
              <div className="flex min-w-0 flex-col">
                <Text weight="medium">{source.name}</Text>
                <Text tone="secondary" size="sm">
                  {t("detail.sourceLine", {
                    subject: source.subject,
                    date: whenLabel(source.date, locale),
                  })}
                </Text>
              </div>
            </div>
            {quote === undefined || quote === "" ? null : (
              // The email's language is not known: no `lang`, the marks are the member's.
              <SenderSentence>{t("detail.quote", { quote })}</SenderSentence>
            )}
            {task.replied === null || task.replied === undefined ? null : (
              <Text tone="secondary" size="sm">
                {t("detail.yourReply", { date: whenLabel(task.replied.at, locale) })}
              </Text>
            )}
            <div>
              <ButtonLink variant="link" href={source.href} onClick={openEmail}>
                {t("detail.openEmail")}
              </ButtonLink>
            </div>
          </>
        )}
      </section>

      {history.length === 0 ? null : (
        <section className="flex flex-col gap-2">
          <Heading level={3} size="section">
            {t("detail.history")}
          </Heading>
          <ol className="flex flex-col gap-2">
            {shown.map((entry, index) => (
              <li key={entry.id} className="flex flex-col gap-2">
                {folded && index === 1 ? (
                  <div>
                    <Button
                      variant="link"
                      onClick={() => {
                        setExpanded(true);
                      }}
                    >
                      {t("history.showAll", { total: history.length })}
                    </Button>
                  </div>
                ) : null}
                <div className="flex items-baseline gap-2">
                  <Dot signal={DOT[entry.actor]} />
                  {/* The time stays on one line, so every sentence starts where the others do. */}
                  <span className="shrink-0 whitespace-nowrap">
                    <Text tone="tertiary" size="sm" as="span">
                      {historyStamp(entry.at, locale)}
                    </Text>
                  </span>
                  <span className="min-w-0">
                    <Text as="span">
                      {historyLine(entry, task.verb, {
                        t,
                        locale,
                        me,
                        memberName: named,
                        sourceName: source?.name,
                      })}
                    </Text>
                  </span>
                </div>
              </li>
            ))}
          </ol>
        </section>
      )}
    </>
  );
}
