import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ruleNoticeRef, useScoutChat } from "~/lib/scout-chat";
import { useShortcut } from "~/lib/shortcuts";
import { Button, Note, OverflowActions, useUndoToast, Visor } from "~/lib/ui";

import { useInboxes } from "../../api/queries";
import { useNoticeAnswers, useRuleNotices, type RuleNotice } from "../../api/rules";
import { streamName, type Translate } from "../../model";
import { ruleStream } from "../../rules-text";
import { noticeSender } from "../../scout-subjects";

import { SmartRuleConfirm, type SmartRuleDraft } from "./SmartRuleConfirm";

export interface ScoutNoticedProps {
  workspaceId: string;
}

/** What the question calls the sender: their name when they give one, else the address. */
function nameOf(notice: RuleNotice): string {
  const name = notice.sender_name?.trim();
  return name === undefined || name === "" ? notice.sender : name;
}

/**
 * The search for the mail a notice counts: the sender's, in the notice's
 * window (`from:` and `newer_than:`, the search grammar's own operators),
 * narrowed to the kind of its mail a notice is about (`subject:"…"`, from
 * the server) when it is about one.
 */
function noticeQuery(notice: RuleNotice): string {
  const sender = /\s/u.test(notice.sender) ? `"${notice.sender}"` : notice.sender;
  const query = `from:${sender} newer_than:${String(notice.evidence.window_days)}d`;
  const narrow = notice.pattern?.search.trim() ?? "";
  return narrow === "" ? query : `${query} ${narrow}`;
}

/** Longest example subject the sentence quotes, in characters. */
const EXAMPLE_CHARS = 60;

/**
 * The real subject a notice about one kind of a sender's mail quotes, cut
 * to fit the sentence; none for a notice about all of it.
 */
function exampleOf(notice: RuleNotice): string | undefined {
  const example = notice.pattern?.example.trim() ?? "";
  if (example === "") {
    return undefined;
  }
  // Whole characters as a reader sees them, so an accent or emoji is never
  // cut in two.
  const chars = Array.from(
    new Intl.Segmenter(undefined, { granularity: "grapheme" }).segment(example),
    (part) => part.segment,
  );
  if (chars.length <= EXAMPLE_CHARS) {
    return example;
  }
  // Cut at a space when there is one late enough, never mid-word.
  const cut = chars.slice(0, EXAMPLE_CHARS - 1).join("");
  const space = cut.lastIndexOf(" ");
  return `${(space > EXAMPLE_CHARS / 2 ? cut.slice(0, space) : cut).trimEnd()}…`;
}

/** What Scout noticed, as one sentence and its question, in the reader's words. */
function sentence(t: Translate, locale: string, notice: RuleNotice): string {
  const { evidence } = notice;
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const sender = noticeSender(notice);
  const example = exampleOf(notice);
  const counts = {
    count: evidence.received,
    n: number(evidence.received),
    sender,
    days: number(evidence.window_days),
    opened: number(evidence.opened),
    archived: number(evidence.archived_unread),
    example: example ?? "",
  };
  // A notice about one kind of the sender's mail names it by a real subject.
  const like = example === undefined ? "" : "Like";
  const cadence =
    evidence.cadence === undefined || evidence.cadence === null
      ? ""
      : ` ${t(`noticed.cadence.${evidence.cadence}`)}`;
  if (notice.kind === "archive_on_arrival") {
    return `${t(`noticed.archived${like}`, counts)}${cadence} ${t("noticed.archiveAsk")}`;
  }
  const stream = ruleStream(notice.actions) ?? "updates";
  return `${t(`noticed.keepOffHome${like}`, counts)}${cadence} ${t("noticed.keepOffHomeAsk", {
    stream: streamName(t, stream),
  })}`;
}

/**
 * "Scout noticed" on Home, right after the first group (campbooks-cloud#169,
 * #274, #347): one suggestion at a time from `conversations.list_rule_notices`,
 * titled with "n of N", what Scout noticed in numbers in the body font (for
 * one kind of a sender's mail, #356, with a real subject of it as an
 * example), "See the N emails" (the search for them, narrowed to that kind),
 * and one row of buttons of one style:
 * the notice's own verb ("Archive them…"), Something else… (with the visor)
 * and Not now (with Undo), then More: Don't suggest this (with Undo) and Ask
 * Scout (the chat about the suggestion, where one is mounted). The row folds
 * into More when it would wrap. Both of the first two open the confirmation,
 * which asks what should happen to the sender's mail and alone saves.
 * Nothing while the list loads or fails, or when there is none.
 */
export function ScoutNoticed({ workspaceId }: ScoutNoticedProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const undo = useUndoToast();
  const notices = useRuleNotices(workspaceId);
  const inboxes = useInboxes(workspaceId);
  const { answer, clear } = useNoticeAnswers(workspaceId);
  // Answered here: hidden at once, before the list comes back without them.
  const [hidden, setHidden] = useState<ReadonlySet<string>>(new Set());
  const [draft, setDraft] = useState<SmartRuleDraft | null>(null);
  const shown = (notices.data ?? []).filter((notice) => !hidden.has(notice.id));
  const notice = shown[0];
  // Ask Scout (campbooks-cloud#305): the chat about this suggestion; the
  // focus comes back to where it was asked from on close.
  const scout = useScoutChat();
  const ask = () => {
    if (notice !== undefined) {
      const active = document.activeElement;
      scout?.(ruleNoticeRef(notice.id), {
        from: active instanceof HTMLElement && active !== document.body ? active : null,
      });
    }
  };
  // See the N emails: the search for the sender's mail in the window counted.
  const seeEmails = () => {
    if (notice !== undefined) {
      void navigate({ to: `/w/${workspaceId}/search`, search: { q: noticeQuery(notice) } });
    }
  };

  const hide = (id: string, on: boolean) => {
    setHidden((current) => {
      const next = new Set(current);
      if (on) {
        next.add(id);
      } else {
        next.delete(id);
      }
      return next;
    });
  };
  const respond = (current: RuleNotice, kind: "not_now" | "never") => {
    hide(current.id, true);
    answer.mutate(
      { noticeId: current.id, answer: kind },
      {
        onSuccess: () => {
          undo.push({
            label: t(kind === "never" ? "noticed.neverDone" : "noticed.notNowDone"),
            undo: () => {
              hide(current.id, false);
              clear.mutateAsync({ noticeId: current.id }).catch(() => {
                hide(current.id, true);
                undo.notify(t("noticed.failed"));
              });
            },
          });
        },
        onError: () => {
          hide(current.id, false);
          undo.notify(t("noticed.failed"));
        },
      },
    );
  };

  const confirm = (
    <SmartRuleConfirm
      workspaceId={workspaceId}
      draft={draft}
      onClose={() => {
        setDraft(null);
      }}
    />
  );
  const inbox = inboxes.data?.find((item) => item.id === notice?.inbox_id);
  const sharedInbox = notice?.shared === true && inbox !== undefined ? inbox.address : undefined;
  // The verb button starts on Scout's guess; Something else… on nothing.
  const setUp = (start: "guess" | "none") => {
    if (notice === undefined) {
      return;
    }
    setDraft({
      inboxId: notice.inbox_id,
      conditions: notice.conditions,
      actions: notice.actions,
      title: t("smartRule.titleNotice", { sender: notice.sender }),
      origin: "suggestion",
      noticeId: notice.id,
      sharedInbox,
      goal: { kind: notice.kind, sender: nameOf(notice), address: notice.sender, start },
    });
  };
  // Shift + F, O, L and D press the four buttons, while no confirmation is
  // open (campbooks-cloud#282).
  const live = notice !== undefined && draft === null;
  useShortcut(
    "inbox.noticed.apply",
    () => {
      setUp("guess");
    },
    { enabled: live },
  );
  useShortcut(
    "inbox.noticed.other",
    () => {
      setUp("none");
    },
    { enabled: live },
  );
  useShortcut(
    "inbox.noticed.not_now",
    () => {
      if (notice !== undefined) {
        respond(notice, "not_now");
      }
    },
    { enabled: live },
  );
  useShortcut(
    "inbox.noticed.never",
    () => {
      if (notice !== undefined) {
        respond(notice, "never");
      }
    },
    { enabled: live },
  );
  useShortcut("inbox.noticed.ask", ask, { enabled: live && scout !== undefined });
  useShortcut("inbox.noticed.see", seeEmails, { enabled: live });
  // The confirmation stays where it is when the suggestion it answers goes
  // (saved): its result shows to the end.
  if (notice === undefined) {
    return (
      <>
        {null}
        {confirm}
      </>
    );
  }
  const verb =
    notice.kind === "archive_on_arrival"
      ? t("noticed.verbArchive")
      : t("noticed.verbFile", { stream: streamName(t, ruleStream(notice.actions) ?? "updates") });
  return (
    <>
      <Note
        visor="happy"
        label={t("noticed.title")}
        title={t("noticed.title")}
        meta={t("noticed.position", { n: 1, total: shown.length })}
        after={
          <Button
            variant="link"
            icon="arrow-right"
            shortcut="inbox.noticed.see"
            onClick={seeEmails}
          >
            {t("noticed.see", { count: notice.evidence.received })}
          </Button>
        }
        actions={
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <Button
              variant="secondary"
              shortcut="inbox.noticed.apply"
              onClick={() => {
                setUp("guess");
              }}
            >
              {verb}
            </Button>
            <OverflowActions
              label={t("noticed.more")}
              buttons
              actions={[
                {
                  id: "other",
                  label: t("noticed.other"),
                  icon: <Visor size="xs" />,
                  shortcut: "inbox.noticed.other",
                  onSelect: () => {
                    setUp("none");
                  },
                },
                {
                  id: "not-now",
                  label: t("noticed.notNow"),
                  icon: "clock",
                  shortcut: "inbox.noticed.not_now",
                  onSelect: () => {
                    respond(notice, "not_now");
                  },
                },
              ]}
              more={[
                {
                  id: "never",
                  label: t("noticed.never"),
                  icon: "close",
                  shortcut: "inbox.noticed.never",
                  onSelect: () => {
                    respond(notice, "never");
                  },
                },
                ...(scout === undefined
                  ? []
                  : [
                      {
                        id: "ask",
                        label: t("noticed.ask"),
                        icon: <Visor size="xs" />,
                        shortcut: "inbox.noticed.ask",
                        onSelect: ask,
                      },
                    ]),
              ]}
            />
          </div>
        }
      >
        {sentence(t, i18n.language, notice)}
        {sharedInbox === undefined ? null : ` ${t("noticed.shared", { inbox: sharedInbox })}`}
      </Note>
      {confirm}
    </>
  );
}
