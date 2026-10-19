import { useTranslation } from "react-i18next";

import {
  Button,
  ConversationRow,
  EmptyState,
  IconButton,
  initialsOf,
  SkeletonList,
  Tag,
  Text,
} from "~/lib/ui";

import { DRAFTS_MAX, type Draft } from "../../api/drafts";
import { displayName, listTime, type Translate } from "../../model";

/** A draft the member can still change: not one on its way out. */
function editable(draft: Draft): boolean {
  return draft.state === "draft" || draft.state === "failed";
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
  /** For "now": tests and stories pin the clock. */
  now?: Date | undefined;
}

/**
 * Your drafts, newest first (`conversations.list_drafts`: no paging, at most
 * 50). Each row opens its composer; Delete draft sits beside it.
 */
export function DraftsList({ drafts, failed, onRetry, onOpen, onDelete, now }: DraftsListProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;

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
  if (shown.length === 0) {
    return <EmptyState title={t("empty.drafts")} visor="idle" visorSize="lg" />;
  }
  return (
    <div className="flex flex-col gap-0-5 p-1-5">
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
