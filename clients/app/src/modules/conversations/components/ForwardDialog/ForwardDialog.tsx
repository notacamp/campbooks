import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { useDebounced } from "~/lib/hooks/use-debounced";
import { Banner, Dialog, DialogChoices, SearchField, Text } from "~/lib/ui";

import {
  changedElsewhere,
  createDraft,
  draftKeys,
  saveDraft,
  useConversationDrafts,
  type Draft,
} from "../../api/drafts";
import {
  useConversation,
  useSenders,
  type MessageView,
  type NeedView,
  type Sender,
} from "../../api/queries";
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

/**
 * A forward draft nobody has written in yet: no text of the member's own,
 * no copies, no files, at most the one person it was addressed to. Only
 * such a draft is addressed again; any other stays as it was.
 */
function pristine(draft: Draft): boolean {
  return (
    draft.state === "draft" &&
    draft.body.trim() === "" &&
    draft.cc.length === 0 &&
    draft.bcc.length === 0 &&
    draft.attachments.length === 0 &&
    draft.to.length <= 1
  );
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
  const drafts = useConversationDrafts(workspaceId, conversationId);
  const [typed, setTyped] = useState("");
  // The person picked: the forward starts once the email and its drafts are here.
  const [chosen, setChosen] = useState<Sender | undefined>();
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const text = typed.trim();
  const settled = useDebounced(text, DEBOUNCE_MS);
  const searching = settled.length >= MIN_CHARS;
  // Only people it is safe to write to: the server leaves out no-reply
  // addresses, senders failing their checks and senders under review, and
  // a name typed for one of them finds nobody.
  const top = useSenders(workspaceId, "", { reachable: true });
  const found = useSenders(workspaceId, settled, { enabled: searching, reachable: true });

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

  // The pick a forward was started for (a failed one can be picked again).
  const started = useRef<Sender | null>(null);
  // Which start is current: one given up on (it failed in the meantime)
  // hands nothing to the composer when its draft arrives.
  const attempt = useRef(0);
  // Closed (Esc, Close) while the forward was being started: nothing more.
  const closed = useRef(false);
  useEffect(() => {
    closed.current = false;
    return () => {
      closed.current = true;
    };
  }, []);
  const close = () => {
    closed.current = true;
    onClose();
  };
  // An untouched forward of this email is addressed again rather than
  // doubled; one the member wrote in stays theirs, and a new one starts.
  const start = useEffectEvent((person: Sender, message: MessageView, known: Draft[]) => {
    const to = [{ name: person.name ?? null, address: person.address }];
    const existing = known.find(
      (draft) => draft.kind === "forward" && draft.message_id === message.id && pristine(draft),
    );
    const fresh = () =>
      createDraft(workspaceId, {
        kind: "forward",
        message_id: message.id,
        quoted: forwardQuote(message, i18n.language, words),
        to,
      });
    // Saved with the version and token that came with it; one that changed
    // since (`draft_changed` or `content_changed`, campbooks-cloud#242) is
    // left alone and a new forward starts. Either way it opens in the
    // composer, which shows all of it before Send.
    const saved =
      existing === undefined
        ? fresh()
        : saveDraft(workspaceId, existing.id, {
            to,
            version: existing.version,
            content_token: existing.content_token ?? null,
          }).catch(async (thrown: unknown) => {
            if (!changedElsewhere(thrown)) {
              throw thrown;
            }
            return fresh();
          });
    attempt.current += 1;
    const mine = attempt.current;
    void saved
      .then(async (draft) => {
        // Known at once, so a reopen addresses it again rather than
        // starting a second one, even when closed or given up on meanwhile.
        queryClient.setQueryData<Draft[]>(
          draftKeys.forConversation(workspaceId, conversationId),
          (list) => [draft, ...(list ?? []).filter((each) => each.id !== draft.id)],
        );
        // Closed, or failed while it started: nothing opens.
        if (!closed.current && attempt.current === mine) {
          onForwarded(draft);
        }
        await queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      })
      .catch(() => {
        started.current = null;
        setFailed(true);
        setChosen(undefined);
      })
      .finally(() => {
        setBusy(false);
      });
  });
  const message = needMessageOf(detail.data?.messages ?? [], need);
  // The email or its drafts can't be read: say so, and let another pick try.
  const unavailable =
    detail.isError || drafts.isError || (detail.data !== undefined && message === undefined);
  if (chosen !== undefined && unavailable) {
    setChosen(undefined);
    setBusy(false);
    setFailed(true);
  }
  // Said failed: a start still running is given up on, before its draft can
  // arrive (layout effects run with the commit that shows the failure).
  useLayoutEffect(() => {
    if (failed) {
      attempt.current += 1;
      started.current = null;
    }
  }, [failed]);
  useEffect(() => {
    if (
      chosen !== undefined &&
      chosen !== started.current &&
      message !== undefined &&
      drafts.data !== undefined
    ) {
      started.current = chosen;
      start(chosen, message, drafts.data);
    }
  }, [chosen, message, drafts.data]);

  const pick = (address: string) => {
    const person = listed.find((sender) => sender.address === address);
    if (person === undefined || busy) {
      return;
    }
    setBusy(true);
    setFailed(false);
    setChosen(person);
  };

  return (
    <Dialog
      open
      onClose={close}
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
          // Before the dialog's own keys (it closes on Esc and keeps the
          // arrows for its choices): Enter picks the first match, ↓ goes to
          // the people, Esc clears what was typed first.
          onKeyDownCapture={(event) => {
            if (event.key === "Enter" && text !== "") {
              event.preventDefault();
              const first = listed[0];
              if (first !== undefined) {
                pick(first.address);
              }
            } else if (event.key === "ArrowDown") {
              const choice = event.currentTarget
                .closest("[role='dialog']")
                ?.querySelector<HTMLElement>("[data-dialog-choice]");
              if (choice !== null && choice !== undefined) {
                event.preventDefault();
                choice.focus();
              }
            } else if (event.key === "Escape" && typed !== "") {
              event.preventDefault();
              event.stopPropagation();
              setTyped("");
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
