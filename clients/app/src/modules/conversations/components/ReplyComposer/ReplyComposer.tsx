import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useEffectEvent, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Banner, Button, Text, useUndoToast } from "~/lib/ui";

import { createDraft, draftKeys, useConversationDrafts, type Draft } from "../../api/drafts";
import { useMessageWords, withWords } from "../../api/queries";
import { displayName, forwardQuote, replyQuote, type QuoteWords } from "../../compose/quote";
import { useDeleteDraft } from "../../compose/use-delete-draft";
import type { Translate } from "../../model";
import { DraftComposer } from "../DraftComposer/DraftComposer";
import type { ReplyRequest, ReplySlotContext } from "../Reader/ReplySlot";

/** A draft the member can still change. */
function editable(draft: Draft): boolean {
  return draft.state === "draft" || draft.state === "failed";
}

/** What a draft is, as its title says it: "Reply to Maya Okafor", "Forward". */
function titleOf(draft: Draft, t: Translate) {
  const first = draft.to[0];
  return draft.kind === "reply"
    ? t("compose.title.reply", { name: first === undefined ? "" : displayName(first) })
    : t(`compose.title.${draft.kind}`);
}

/** The reader's reply slot: one argument, so it can be passed as `renderReplySlot`. */
export function renderReplyComposer(context: ReplySlotContext) {
  return <ReplyComposer {...context} />;
}

/**
 * The composer at the end of a conversation. Reply, Reply all and Forward
 * (the reader's buttons and R / A / F) start a draft of the message they
 * name, or reopen the one already started; a draft left open comes back
 * when the conversation does.
 */
export function ReplyComposer({
  workspaceId,
  conversation,
  messages,
  request,
  draftId: openDraftId,
  onClose,
}: ReplySlotContext) {
  const { t, i18n } = useTranslation("conversations");
  const queryClient = useQueryClient();
  const drafts = useConversationDrafts(workspaceId, conversation.id);
  const [draftId, setDraftId] = useState<string | undefined>(openDraftId);
  const [closed, setClosed] = useState(false);
  // Why starting the draft failed: its quote could not be read, or else.
  const [failed, setFailed] = useState<"quote" | "draft" | false>(false);
  // Reading the message's words for the quote (it came `text_unread`).
  const [reading, setReading] = useState(false);
  const deleteDraft = useDeleteDraft(workspaceId);
  const toast = useUndoToast();
  const tr = t as unknown as Translate;
  const started = useRef<number | null>(null);

  const words: QuoteWords = {
    wrote: (date, name) => t("compose.quote.wrote", { date, name }),
    forwarded: t("compose.quote.forwarded"),
    from: t("compose.quote.from"),
    date: t("compose.quote.date"),
    subject: t("compose.quote.subject"),
    to: t("compose.quote.to"),
  };

  // A press of Reply, Reply all or Forward: reopen or start its draft.
  const start = useEffectEvent((request: ReplyRequest, known: Draft[]) => {
    started.current = request.nonce;
    setClosed(false);
    setFailed(false);
    const existing = known.find(
      (draft) =>
        editable(draft) && draft.kind === request.mode && draft.message_id === request.messageId,
    );
    if (existing !== undefined) {
      setDraftId(existing.id);
      return;
    }
    const found = messages.find((m) => m.id === request.messageId);
    if (found === undefined) {
      return;
    }
    // Its words first when the conversation came without them: never an
    // empty quote, silently.
    setReading(found.text_unread === true);
    void withWords(queryClient, workspaceId, conversation.id, found)
      .catch((error: unknown) => {
        setFailed("quote");
        throw error;
      })
      .then(async (message) => {
        setReading(false);
        const quoted =
          request.mode === "forward"
            ? forwardQuote(message, i18n.language, words)
            : replyQuote(message, i18n.language, words);
        const draft = await createDraft(workspaceId, {
          kind: request.mode,
          message_id: message.id,
          quoted,
        }).catch((error: unknown) => {
          setFailed("draft");
          throw error;
        });
        queryClient.setQueryData<Draft[]>(
          draftKeys.forConversation(workspaceId, conversation.id),
          (list) => [draft, ...(list ?? [])],
        );
        setDraftId(draft.id);
        await queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      })
      .catch(() => {
        setReading(false);
      });
  });
  useEffect(() => {
    if (request !== null && started.current !== request.nonce && drafts.data !== undefined) {
      start(request, drafts.data);
    }
  }, [request, drafts.data]);

  const open = drafts.data?.filter(editable) ?? [];
  const draft = open.find((d) => d.id === draftId) ?? (request === null ? open[0] : undefined);
  // The message it quotes, and whether its words were cut (a long email):
  // the composer says the quote holds its first part.
  const quotedMessage = messages.find((m) => m.id === draft?.message_id);
  const quotedWords = useMessageWords(workspaceId, conversation.id, quotedMessage);
  const quoteCut =
    quotedMessage?.quote_cut === true ||
    quotedMessage?.text?.truncated === true ||
    quotedWords.data?.truncated === true;

  if (failed !== false) {
    return (
      <Banner
        tone="danger"
        role="alert"
        title={failed === "quote" ? t("compose.errors.quote") : t("compose.errors.unknown")}
      />
    );
  }
  if (reading) {
    return (
      <div role="status">
        <Text tone="secondary">{t("compose.readingQuote")}</Text>
      </div>
    );
  }
  if (closed) {
    // Every draft left on this conversation, each named and each deletable.
    return open.length === 0 ? null : (
      <ul className="m-0 flex list-none flex-col gap-1 p-0">
        {open.map((chip) => {
          const name = titleOf(chip, tr);
          return (
            <li key={chip.id} className="flex flex-wrap items-center gap-1">
              <Button
                variant="ghost"
                icon="reply"
                onClick={() => {
                  setDraftId(chip.id);
                  setClosed(false);
                }}
              >
                {t("compose.draftChip", { title: name })}
              </Button>
              <Button
                variant="ghost"
                aria-label={t("compose.deleteDraftNamed", { title: name })}
                onClick={() => {
                  void deleteDraft(chip).catch(() => {
                    toast.notify(t("compose.errors.unknown"));
                  });
                }}
              >
                {t("compose.deleteDraft")}
              </Button>
            </li>
          );
        })}
      </ul>
    );
  }
  if (draft === undefined) {
    return null;
  }
  const title = titleOf(draft, tr);
  return (
    <DraftComposer
      key={draft.id}
      workspaceId={workspaceId}
      draft={draft}
      title={title}
      withSubject={draft.kind === "forward"}
      focus={draft.kind === "forward" ? "to" : "body"}
      quotedNote={quoteCut ? t("compose.quoteCut") : undefined}
      onClose={() => {
        setClosed(true);
        onClose();
      }}
      onDone={() => {
        // Sent or deleted: any other draft left here shows as a chip.
        setDraftId(undefined);
        setClosed(true);
        onClose();
      }}
      onUndone={() => {
        setDraftId(draft.id);
        setClosed(false);
      }}
    />
  );
}
