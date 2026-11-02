import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { EmptyState, SkeletonList } from "~/lib/ui";

import { createDraft, useDraft, useSendingInboxes } from "../../api/drafts";
import { DraftComposer } from "../DraftComposer/DraftComposer";

export interface NewMessageProps {
  workspaceId: string;
  /** A draft to reopen (after an undone send). */
  draftId?: string | undefined;
  /** Sent, discarded or closed: back to the inbox. */
  onLeave: () => void;
  /** A send was taken back: reopen its draft here. */
  onReopen: (draftId: string) => void;
}

/** The New message screen: from one of your inboxes (picked in the composer), to anyone. */
export function NewMessage({ workspaceId, draftId, onLeave, onReopen }: NewMessageProps) {
  const { t } = useTranslation("conversations");
  const inboxes = useSendingInboxes(workspaceId);
  const [created, setCreated] = useState<string | undefined>(draftId);
  const [failed, setFailed] = useState(false);
  const creating = useRef(false);
  const draft = useDraft(workspaceId, created);
  const from = inboxes.data?.[0]?.id;

  // Start a draft in the first inbox (unless one was given to reopen): the
  // composer's From moves it to another.
  useEffect(() => {
    if (draftId !== undefined || from === undefined || creating.current) {
      return;
    }
    creating.current = true;
    void createDraft(workspaceId, { kind: "new", inbox_id: from })
      .then((fresh) => {
        setCreated(fresh.id);
      })
      .catch(() => {
        setFailed(true);
      });
  }, [draftId, from, workspaceId]);

  if (inboxes.data?.length === 0) {
    return (
      <section className="flex w-full flex-col items-center p-6">
        <EmptyState title={t("compose.newMessage")} visor="idle" visorSize="lg">
          {t("compose.noInbox")}
        </EmptyState>
      </section>
    );
  }
  if (failed || draft.isError) {
    return (
      <section className="flex w-full flex-col items-center p-6">
        <EmptyState title={t("compose.newMessage")} visor="alert" visorSize="lg">
          {t("compose.errors.unknown")}
        </EmptyState>
      </section>
    );
  }
  return (
    <section className="flex w-full max-w-content flex-col gap-3 p-3 md:p-6">
      {draft.data === undefined ? (
        <SkeletonList rows={3} />
      ) : (
        <DraftComposer
          key={draft.data.id}
          workspaceId={workspaceId}
          draft={draft.data}
          title={t("compose.title.new")}
          withSubject
          focus={draft.data.to.length === 0 ? "to" : "body"}
          onClose={onLeave}
          onDone={onLeave}
          onUndone={() => {
            onReopen(draft.data.id);
          }}
        />
      )}
    </section>
  );
}
