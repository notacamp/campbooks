import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useDebounced } from "~/lib/hooks/use-debounced";
import { Banner, Dialog, DialogChoices, SearchField, Text } from "~/lib/ui";

import { createDraft, draftKeys, type Draft } from "../../api/drafts";
import { useConversation, useSenders, type NeedView, type Sender } from "../../api/queries";
import { forwardQuote, type QuoteWords } from "../../compose/quote";
import { needMessageOf } from "../NeedMessage/NeedMessage";

/** How long typing settles before the server is asked. */
const DEBOUNCE_MS = 250;
/** Shortest text the server searches (`list_senders`). */
const MIN_CHARS = 2;
/** The choices take the number keys, 1 to 9. */
const MAX_CHOICES = 9;

export interface ForwardDialogProps {
  workspaceId: string;
  conversationId: string;
  /** The need whose message is forwarded. */
  need: NeedView;
  /** Whose email it is ("Marta Ferreira"). */
  name: string;
  /** Esc, a click outside or Close: nothing was started. */
  onClose: () => void;
  /** The forward draft, addressed to the person picked, to finish in the composer. */
  onForwarded: (draft: Draft) => void;
}

/** Lower-cased, without accents. */
function fold(text: string): string {
  return text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/** Whether every word typed is in the person's name or address, as the server matches. */
function matches(sender: Sender, typed: string): boolean {
  const haystack = fold(`${sender.address} ${sender.name ?? ""}`);
  return fold(typed)
    .split(/[\s,]+/u)
    .filter((word) => word !== "")
    .every((word) => haystack.includes(word));
}

/**
 * Forward a need's email: a filter box finds the person by name (the people
 * you reply to most before anything is typed, `conversations.list_senders`
 * from two characters), never an address typed from memory. Picking one
 * starts a forward draft addressed to them, quoting the email, and hands it
 * to the composer: nothing is sent from here.
 */
export function ForwardDialog({
  workspaceId,
  conversationId,
  need,
  name,
  onClose,
  onForwarded,
}: ForwardDialogProps) {
  const { t, i18n } = useTranslation("conversations");
  const queryClient = useQueryClient();
  const detail = useConversation(workspaceId, conversationId);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const text = typed.trim();
  const settled = useDebounced(text, DEBOUNCE_MS);
  const searching = settled.length >= MIN_CHARS;
  const top = useSenders(workspaceId);
  const found = useSenders(workspaceId, settled, { enabled: searching });

  // What the server found for the settled text, or, until it answers, the
  // people already here, filtered as it would.
  const answered = searching && settled === text && found.data !== undefined;
  const people: Sender[] =
    text === ""
      ? (top.data ?? [])
      : answered
        ? found.data
        : (top.data ?? []).filter((sender) => matches(sender, text));
  const listed = people.slice(0, MAX_CHOICES);
  const looking = text.length >= MIN_CHARS && !answered && !found.isError;

  const words: QuoteWords = {
    wrote: (date, who) => t("compose.quote.wrote", { date, name: who }),
    forwarded: t("compose.quote.forwarded"),
    from: t("compose.quote.from"),
    date: t("compose.quote.date"),
    subject: t("compose.quote.subject"),
    to: t("compose.quote.to"),
  };

  const pick = (address: string) => {
    const person = listed.find((sender) => sender.address === address);
    const message = needMessageOf(detail.data?.messages ?? [], need);
    if (person === undefined || message === undefined || busy) {
      return;
    }
    setBusy(true);
    setFailed(false);
    void createDraft(workspaceId, {
      kind: "forward",
      message_id: message.id,
      quoted: forwardQuote(message, i18n.language, words),
      to: [{ name: person.name ?? null, address: person.address }],
    })
      .then(async (draft) => {
        queryClient.setQueryData<Draft[]>(
          draftKeys.forConversation(workspaceId, conversationId),
          (list) => [draft, ...(list ?? [])],
        );
        onForwarded(draft);
        await queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      })
      .catch(() => {
        setFailed(true);
      })
      .finally(() => {
        setBusy(false);
      });
  };

  return (
    <Dialog
      open
      onClose={onClose}
      title={t("forward.title", { name })}
      description={t("forward.description")}
    >
      <div className="flex flex-col gap-3">
        <SearchField
          label={t("forward.find")}
          placeholder={t("forward.placeholder")}
          value={typed}
          onValueChange={setTyped}
          onClear={() => {
            setTyped("");
          }}
          inputMode="search"
          variant="page"
          data-autofocus=""
          onKeyDown={(event) => {
            // With text in the box, Enter picks the first match.
            if (text !== "" && event.key === "Enter") {
              event.preventDefault();
              const first = listed[0];
              if (first !== undefined) {
                pick(first.address);
              }
            }
          }}
        />
        {listed.length === 0 ? null : (
          <DialogChoices
            label={t("forward.people")}
            choices={listed.map((sender) => ({
              id: sender.address,
              label: sender.name ?? sender.address,
              detail:
                sender.name === null || sender.name === undefined ? undefined : sender.address,
            }))}
            onChoose={pick}
          />
        )}
        <div role="status">
          {failed ? (
            <Banner tone="danger" title={t("forward.failed")} />
          ) : busy ? (
            <Text tone="secondary">{t("forward.starting")}</Text>
          ) : text !== "" && listed.length === 0 && !looking ? (
            <Text tone="secondary">
              {found.isError ? t("forward.searchFailed") : t("forward.none", { text })}
            </Text>
          ) : text === "" && top.data?.length === 0 ? (
            <Text tone="secondary">{t("forward.nobody")}</Text>
          ) : null}
        </div>
      </div>
    </Dialog>
  );
}
