import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { ruleNoticeRef, useScoutChat } from "~/lib/scout-chat";
import { useShortcut } from "~/lib/shortcuts";
import { Button, Note, Text, useUndoToast } from "~/lib/ui";

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

/** What Scout noticed, as one sentence and its question, in the reader's words. */
function sentence(t: Translate, locale: string, notice: RuleNotice): string {
  const { evidence } = notice;
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const sender = noticeSender(notice);
  const counts = {
    count: evidence.received,
    n: number(evidence.received),
    sender,
    days: number(evidence.window_days),
    opened: number(evidence.opened),
    archived: number(evidence.archived_unread),
  };
  const cadence =
    evidence.cadence === undefined || evidence.cadence === null
      ? ""
      : ` ${t(`noticed.cadence.${evidence.cadence}`)}`;
  if (notice.kind === "archive_on_arrival") {
    return `${t("noticed.archived", counts)}${cadence} ${t("noticed.archiveAsk")}`;
  }
  const stream = ruleStream(notice.actions) ?? "updates";
  return `${t("noticed.keepOffHome", counts)}${cadence} ${t("noticed.keepOffHomeAsk", {
    stream: streamName(t, stream),
  })}`;
}

/**
 * "Scout noticed" on Home, right after the first group (campbooks-cloud#169,
 * #274): one suggestion at a time from `conversations.list_rule_notices`, with
 * "n of N", what Scout noticed in numbers, and buttons of one style: the
 * notice's own verb ("Archive them…"), Something else… (with the visor), Not
 * now and Don't suggest this (each with Undo), and Ask Scout (the chat about
 * the suggestion, where one is mounted). Both of the first two open the
 * confirmation, which asks what should happen to the sender's mail and alone
 * saves. Nothing while the list loads or fails, or when there is none.
 */
export function ScoutNoticed({ workspaceId }: ScoutNoticedProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
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
  // focus comes back to the button on close.
  const scout = useScoutChat();
  const askButton = useRef<HTMLButtonElement>(null);
  const ask = () => {
    if (notice !== undefined) {
      scout?.(ruleNoticeRef(notice.id), { from: askButton.current });
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
        actions={
          <>
            <Button
              variant="secondary"
              shortcut="inbox.noticed.apply"
              onClick={() => {
                setUp("guess");
              }}
            >
              {verb}
            </Button>
            <Button
              variant="secondary"
              shortcut="inbox.noticed.other"
              ai
              onClick={() => {
                setUp("none");
              }}
            >
              {t("noticed.other")}
            </Button>
            <Button
              variant="secondary"
              shortcut="inbox.noticed.not_now"
              onClick={() => {
                respond(notice, "not_now");
              }}
            >
              {t("noticed.notNow")}
            </Button>
            <Button
              variant="secondary"
              shortcut="inbox.noticed.never"
              onClick={() => {
                respond(notice, "never");
              }}
            >
              {t("noticed.never")}
            </Button>
            {scout === undefined ? null : (
              <Button
                ref={askButton}
                variant="secondary"
                shortcut="inbox.noticed.ask"
                ai
                onClick={ask}
              >
                {t("noticed.ask")}
              </Button>
            )}
          </>
        }
      >
        <strong>{t("noticed.title")}</strong>{" "}
        <Text as="span" tone="tertiary" size="sm">
          {t("noticed.position", { n: 1, total: shown.length })}
        </Text>{" "}
        {sentence(t, i18n.language, notice)}
        {sharedInbox === undefined ? null : ` ${t("noticed.shared", { inbox: sharedInbox })}`}
      </Note>
      {confirm}
    </>
  );
}
