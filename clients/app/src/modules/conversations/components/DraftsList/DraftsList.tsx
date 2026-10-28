import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  ConversationRow,
  EmptyState,
  IconButton,
  initialsOf,
  SkeletonList,
  sendTimeLabel,
  Tag,
  Text,
} from "~/lib/ui";

import { DRAFTS_MAX, type Draft } from "../../api/drafts";

import { displayName, listTime, type Translate } from "../../model";

/** A draft the member can still change: not one on its way out. */
function editable(draft: Draft): boolean {
  return draft.state === "draft" || draft.state === "failed";
}

/** How far ahead a scheduled send must be to be listed: not one in its undo window. */
const SCHEDULED_AHEAD_MS = 2 * 60_000;

/** A send scheduled for later (campbooks-cloud#392), soonest first. */
function scheduledLater(drafts: readonly Draft[], now: Date): Draft[] {
  return drafts
    .filter(
      (draft) =>
        draft.state === "scheduled" &&
        typeof draft.send_at === "string" &&
        new Date(draft.send_at).getTime() > now.getTime() + SCHEDULED_AHEAD_MS,
    )
    .toSorted((a, b) => (a.send_at ?? "").localeCompare(b.send_at ?? ""));
}

/** The draft's words on one line, cut short: its first 140 characters. */
function snippetOf(draft: Draft): string {
  const text = draft.body.replaceAll(/\s+/gu, " ").trim();
  return text.length > 140 ? `${text.slice(0, 139)}…` : text;
}

/** Who a draft is to: the first name, "+2" for the rest (a number, not copy), or none. */
function toLine(t: Translate, draft: Draft): string {
  const [first, ...others] = [...draft.to, ...draft.cc, ...draft.bcc];
  if (first === undefined) {
    return t("drafts.noRecipients");
  }
  const name = displayName(first);
  return others.length === 0 ? name : `${name} +${String(others.length)}`;
}

export interface DraftsListProps {
  /** `undefined` while they load. */
  drafts: readonly Draft[] | undefined;
  failed: boolean;
  onRetry: () => void;
  /** Open the composer on a draft. */
  onOpen: (draft: Draft) => void;
  /** Delete a draft (the caller offers Undo). */
  onDelete: (draft: Draft) => void;
  /** Take back a scheduled send: it becomes a draft again. */
  onCancelSend?: ((draft: Draft) => void) | undefined;
  /** Take back a scheduled send and open its draft in the composer. */
  onCancelAndEdit?: ((draft: Draft) => void) | undefined;
  /** For "now": tests and stories pin the clock. */
  now?: Date | undefined;
}

/**
 * Your drafts, newest first (`conversations.list_drafts`: no paging, at most
 * 50). Each row opens its composer; Delete draft sits beside it.
 */
export function DraftsList({
  drafts,
  failed,
  onRetry,
  onOpen,
  onDelete,
  onCancelSend,
  onCancelAndEdit,
  now,
}: DraftsListProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  // The scheduled send whose row was opened: it asks before taking it back.
  const [asking, setAsking] = useState<string | undefined>();

  if (drafts === undefined) {
    return failed ? (
      <EmptyState
        title={t("drafts.errorTitle")}
        visor="thinking"
        visorSize="lg"
        action={<Button onClick={onRetry}>{t("error.retry")}</Button>}
      >
        {t("error.listBody")}
      </EmptyState>
    ) : (
      <SkeletonList rows={6} />
    );
  }
  const shown = drafts.filter((draft) => editable(draft));
  const later = scheduledLater(drafts, now ?? new Date());
  if (shown.length === 0 && later.length === 0) {
    return <EmptyState title={t("empty.drafts")} visor="idle" visorSize="lg" />;
  }
  return (
    <div className="flex flex-col gap-0-5 p-1-5">
      {later.length === 0 ? null : (
        <section aria-label={t("drafts.scheduled")} className="flex flex-col gap-0-5">
          <Text tone="secondary" size="sm" weight="semibold">
            {t("drafts.scheduled")}
          </Text>
          {later.map((draft) => {
            const first = draft.to[0];
            const title = draft.subject === "" ? t("reader.noSubject") : draft.subject;
            const leaves = t("drafts.leaves", {
              time: sendTimeLabel(draft.send_at ?? "", i18n.language),
            });
            return (
              <div key={draft.id} className="flex flex-col gap-1">
                <div className="flex items-start gap-1">
                  <div className="min-w-0 flex-1">
                    <ConversationRow
                      id={`scheduled-${draft.id}`}
                      from={toLine(t, draft)}
                      chips={<Tag>{leaves}</Tag>}
                      avatar={{
                        kind: "person",
                        ...(first === undefined
                          ? {}
                          : { initials: initialsOf(displayName(first)) }),
                      }}
                      time={listTime(draft.updated_at, i18n.language, now)}
                      dateTime={draft.updated_at}
                      subject={title}
                      snippet={snippetOf(draft)}
                      onSelect={() => {
                        setAsking(draft.id);
                      }}
                    />
                  </div>
                  {onCancelSend === undefined ? null : (
                    <IconButton
                      label={t("drafts.cancelSendNamed", { title })}
                      icon="close"
                      onClick={() => {
                        onCancelSend(draft);
                      }}
                    />
                  )}
                </div>
                {asking === draft.id && onCancelAndEdit !== undefined ? (
                  <div className="flex flex-wrap items-center gap-1 px-2 pb-1" role="group">
                    <Text tone="secondary" size="sm">
                      {t("drafts.editScheduled")}
                    </Text>
                    <Button
                      variant="primary"
                      onClick={() => {
                        setAsking(undefined);
                        onCancelAndEdit(draft);
                      }}
                    >
                      {t("drafts.cancelAndEdit")}
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={() => {
                        setAsking(undefined);
                      }}
                    >
                      {t("drafts.keepScheduled")}
                    </Button>
                  </div>
                ) : null}
              </div>
            );
          })}
          {shown.length === 0 ? null : (
            <Text tone="secondary" size="sm" weight="semibold">
              {t("drafts.label")}
            </Text>
          )}
        </section>
      )}
      {shown.map((draft) => {
        const first = draft.to[0];
        const title = draft.subject === "" ? t("reader.noSubject") : draft.subject;
        return (
          <div key={draft.id} className="flex items-start gap-1">
            <div className="min-w-0 flex-1">
              <ConversationRow
                id={`draft-${draft.id}`}
                from={toLine(t, draft)}
                chips={<Tag>{t("drafts.tag")}</Tag>}
                avatar={{
                  kind: "person",
                  ...(first === undefined ? {} : { initials: initialsOf(displayName(first)) }),
                }}
                time={listTime(draft.updated_at, i18n.language, now)}
                dateTime={draft.updated_at}
                subject={title}
                snippet={snippetOf(draft)}
                onSelect={() => {
                  onOpen(draft);
                }}
              />
            </div>
            <IconButton
              label={t("compose.deleteDraftNamed", { title })}
              icon="trash"
              onClick={() => {
                onDelete(draft);
              }}
            />
          </div>
        );
      })}
      {drafts.length >= DRAFTS_MAX ? (
        <Text tone="secondary" size="sm">
          {t("drafts.cap", { count: DRAFTS_MAX })}
        </Text>
      ) : null}
    </div>
  );
}
