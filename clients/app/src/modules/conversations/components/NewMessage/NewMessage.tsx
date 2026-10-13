import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { EmptyState, Select, SkeletonList } from "~/lib/ui";

import { createDraft, deleteDraft, useDraft, useSendingInboxes, type Inbox } from "../../api/drafts";
import { addressLine } from "../../compose/quote";
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

function fromLine(inbox: Inbox): string {
  return addressLine({ name: inbox.display_name ?? null, address: inbox.address });
}

/** The New message screen: From one of your inboxes, to anyone. */
export function NewMessage({ workspaceId, draftId, onLeave, onReopen }: NewMessageProps) {
  const { t } = useTranslation("conversations");
  const inboxes = useSendingInboxes(workspaceId);
  const [inboxId, setInboxId] = useState<string | undefined>(undefined);
  const [created, setCreated] = useState<string | undefined>(draftId);
  const [failed, setFailed] = useState(false);
  const creating = useRef<string | null>(null);
  const draft = useDraft(workspaceId, created);
  const from = inboxId ?? inboxes.data?.[0]?.id;

  // Start a draft in the chosen inbox (unless one was given to reopen).
  useEffect(() => {
    if (draftId !== undefined || from === undefined || creating.current === from) {
      return;
    }
    const previous = creating.current === null ? undefined : created;
    creating.current = from;
    void createDraft(workspaceId, { kind: "new", inbox_id: from })
      .then((fresh) => {
        setCreated(fresh.id);
        if (previous !== undefined) {
          // The draft of the inbox left behind was never written in.
          void deleteDraft(workspaceId, previous);
        }
      })
      .catch(() => {
        setFailed(true);
      });
  }, [created, draftId, from, workspaceId]);

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
      {inboxes.data !== undefined && inboxes.data.length > 1 && draftId === undefined ? (
        <Select
          label={t("compose.from")}
          value={from ?? ""}
          onChange={setInboxId}
          options={inboxes.data.map((inbox) => ({ value: inbox.id, label: fromLine(inbox) }))}
        />
      ) : null}
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
