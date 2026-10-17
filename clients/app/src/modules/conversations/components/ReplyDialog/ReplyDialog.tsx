import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  AskLine,
  Banner,
  Button,
  Dialog,
  DialogChoices,
  ReplyDraft,
  ReplyQuote,
  Text,
  useUndoToast,
  type DialogResult,
} from "~/lib/ui";

import {
  cancelSend,
  createDraft,
  draftKeys,
  saveDraft,
  sendDraft,
  useConversationDrafts,
  type Draft,
} from "../../api/drafts";
import { useConversation, type MessageView } from "../../api/queries";
import { useReplySuggestions, writeReply } from "../../api/replies";
import { displayName, replyQuote, type QuoteWords } from "../../compose/quote";
import type { Translate } from "../../model";

/** A draft whose send was taken back in time, to pick up again. */
export interface ReplyResume {
  draft: Draft;
  /** What it was written for, so a rewrite keeps to it. */
  intent: string;
}

export interface ReplyDialogProps {
  workspaceId: string;
  /** The conversation answered. */
  conversationId: string;
  /** The person answered, when the caller knows them ("Marta Ferreira"). */
  name?: string | undefined;
  /** Start on this draft (its send was undone). */
  resume?: ReplyResume | undefined;
  /** Esc, a click outside, Close, or the result has shown: nothing more to do here. */
  onClose: () => void;
  /** "Write my own": the plain composer, as before. */
  onWriteOwn: () => void;
  /** "Open in composer": Scout's text, saved as a reply draft. */
  onOpenDraft: (draft: Draft) => void;
  /** Sent: it leaves when the undo window ends. */
  onSent?: ((draft: Draft) => void) | undefined;
  /** The send was taken back in time: offer the draft again. */
  onUndone?: ((resume: ReplyResume) => void) | undefined;
}

type Stage = "choose" | "writing" | "draft";

/** Longest intent or change the server takes, in characters. */
const MAX_ASK_CHARS = 300;

/** Longest quote shown from the message answered, in characters. */
const QUOTE_CHARS = 280;

/** Who a reply to `message` goes to: the server sends it there (Reply-To first). */
function recipientOf(message: MessageView) {
  return message.direction === "outbound" ? message.to[0] : (message.reply_to[0] ?? message.from);
}

/** How long the send can still be undone, from its `send_at` (at least a second). */
function undoWindow(sendAt: string | null | undefined): number | undefined {
  return sendAt === null || sendAt === undefined
    ? undefined
    : Math.max(1000, new Date(sendAt).getTime() - Date.now());
}

/** "Marta Ferreira <marta@casa-lume.example>", or the address alone. */
function addressLine(party: { name?: string | null; address: string }): string {
  const name = party.name?.trim();
  return name === undefined || name === "" ? party.address : `${name} <${party.address}>`;
}

/** The first name, for the title ("Reply to Marta"). */
function firstName(name: string): string {
  return name.includes("@") ? name : (name.split(/\s+/u, 1)[0] ?? name);
}

/**
 * Reply: Scout's likely replies for the newest message, a line to say it in
 * your words, and "Write my own". A pick asks Scout for the draft (only
 * then), shown editable with Shorter and Change…; Send saves it as a reply
 * draft and sends it with the usual undo window. Nothing is created before
 * Send or Open in composer, and Scout's text is only ever shown as text.
 */
export function ReplyDialog({
  workspaceId,
  conversationId,
  name,
  resume,
  onClose,
  onWriteOwn,
  onOpenDraft,
  onSent,
  onUndone,
}: ReplyDialogProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const queryClient = useQueryClient();
  const undo = useUndoToast();
  const detail = useConversation(workspaceId, conversationId);
  const messages = detail.data?.messages ?? [];
  // The newest message, as the reader's Reply answers it.
  const answered = messages.at(-1);
  // Who it really goes to, never only the sender's display name: a Reply-To
  // elsewhere is said out loud.
  const recipient = answered === undefined ? undefined : recipientOf(answered);
  const person = recipient === undefined ? (name ?? "") : displayName(recipient);
  const elsewhere =
    answered?.direction === "inbound" &&
    answered.reply_to[0] !== undefined &&
    answered.reply_to[0].address.toLowerCase() !== answered.from.address.toLowerCase()
      ? answered.reply_to[0]
      : undefined;
  const drafts = useConversationDrafts(workspaceId, conversationId);
  const suggestions = useReplySuggestions(workspaceId, conversationId, answered?.id, {
    enabled: resume === undefined,
  });

  const [stage, setStage] = useState<Stage>(resume === undefined ? "choose" : "draft");
  const [intent, setIntent] = useState(resume?.intent ?? "");
  const [text, setText] = useState(resume?.draft.body ?? "");
  const [saved, setSaved] = useState<Draft | undefined>(resume?.draft);
  const [rewriting, setRewriting] = useState<"shorter" | "change" | undefined>();
  const [problem, setProblem] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<DialogResult | undefined>();
  // Answers to a question no longer asked (Back, another pick) are dropped.
  const asking = useRef(0);
  // One send or save at a time, even for two quick ⌘↵.
  const working = useRef(false);

  const unavailable = problemCode(suggestions.error) === "reply_unavailable";
  const words: QuoteWords = {
    wrote: (date, who) => t("compose.quote.wrote", { date, name: who }),
    forwarded: t("compose.quote.forwarded"),
    from: t("compose.quote.from"),
    date: t("compose.quote.date"),
    subject: t("compose.quote.subject"),
    to: t("compose.quote.to"),
  };

  const failure = (thrown: unknown): string => {
    switch (problemCode(thrown)) {
      case "reply_unavailable": {
        return "unavailable";
      }
      case "rate_limited": {
        return "errors.limit";
      }
      case "ai_daily_limit_reached": {
        return "errors.dailyLimit";
      }
      default: {
        return "errors.write";
      }
    }
  };

  const write = (asked: string, how?: { shorter?: boolean; change?: string }) => {
    if (answered === undefined) {
      return;
    }
    asking.current += 1;
    const ticket = asking.current;
    const rewrite = how !== undefined;
    setProblem(undefined);
    if (rewrite) {
      setRewriting(how.shorter === true ? "shorter" : "change");
    } else {
      setIntent(asked);
      setStage("writing");
    }
    void writeReply(workspaceId, conversationId, {
      message_id: answered.id,
      intent: asked,
      ...(rewrite
        ? { draft: text, shorter: how.shorter === true, change: how.change ?? null }
        : {}),
    })
      .then((written) => {
        if (asking.current !== ticket) {
          return;
        }
        setText(written);
        setStage("draft");
      })
      .catch((thrown: unknown) => {
        if (asking.current !== ticket) {
          return;
        }
        setProblem(failure(thrown));
        // A failed first draft goes back to the choices; a failed rewrite
        // keeps the text the member had.
        if (!rewrite) {
          setStage("choose");
        }
      })
      .finally(() => {
        if (asking.current === ticket) {
          setRewriting(undefined);
        }
      });
  };

  const back = () => {
    asking.current += 1;
    setRewriting(undefined);
    setProblem(undefined);
    setStage("choose");
  };

  /**
   * The reply draft holding what is on screen: the one this dialog saved,
   * else an empty reply draft of the same message (never one with words the
   * member wrote), else a new one.
   */
  const keep = async (): Promise<Draft | undefined> => {
    const reusable =
      saved ??
      drafts.data?.find(
        (d) =>
          d.kind === "reply" &&
          d.state === "draft" &&
          d.message_id === answered?.id &&
          d.body.trim() === "",
      );
    let draft: Draft;
    if (reusable !== undefined) {
      draft = await saveDraft(workspaceId, reusable.id, {
        version: reusable.version,
        body: text,
      });
    } else if (answered === undefined) {
      return undefined;
    } else {
      draft = await createDraft(workspaceId, {
        kind: "reply",
        message_id: answered.id,
        body: text,
        quoted: replyQuote(answered, i18n.language, words),
      });
    }
    setSaved(draft);
    return draft;
  };

  const send = async () => {
    if (working.current || text.trim() === "") {
      return;
    }
    working.current = true;
    setBusy(true);
    setProblem(undefined);
    const handle = undo.pending({ label: t("reply.pending", { name: person }) });
    try {
      const draft = await keep();
      if (draft === undefined) {
        throw new Error("nothing to answer yet");
      }
      const scheduled = await sendDraft(workspaceId, draft.id, draft.version);
      const window = undoWindow(scheduled.send_at);
      const asked = intent;
      handle.done({
        label: t("reply.sending", { name: person }),
        durationMs: window,
        undo: () => {
          void cancelSend(workspaceId, scheduled.id)
            .then((back) => {
              undo.notify(t("reply.undone"));
              void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
              onUndone?.({ draft: back, intent: asked });
            })
            .catch(() => {
              undo.notify(t("reply.tooLate"));
            });
        },
      });
      void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      setResult({
        title: t("reply.result", { name: person }),
        detail: t("reply.resultDetail"),
      });
      onSent?.(scheduled);
    } catch {
      handle.fail({ label: t("reply.errors.send", { name: person }) });
      setProblem("errors.send");
    } finally {
      working.current = false;
      setBusy(false);
    }
  };

  const openInComposer = async () => {
    if (working.current) {
      return;
    }
    working.current = true;
    setBusy(true);
    try {
      const draft = await keep();
      if (draft === undefined) {
        throw new Error("nothing to answer yet");
      }
      // The composer finds it at once (it reopens the reply draft of the
      // newest message).
      queryClient.setQueryData<Draft[]>(
        draftKeys.forConversation(workspaceId, conversationId),
        (list) => [draft, ...(list ?? []).filter((d) => d.id !== draft.id)],
      );
      onOpenDraft(draft);
    } catch {
      setProblem("errors.open");
    } finally {
      working.current = false;
      setBusy(false);
    }
  };

  const drafting = stage === "draft";
  useShortcut(
    "compose.send",
    () => {
      void send();
    },
    { enabled: drafting && rewriting === undefined && result === undefined },
  );

  const intents = suggestions.data?.intents ?? [];
  // The message answered, the one the draft replies to.
  const quote =
    answered?.text?.content === undefined || answered.text.content.trim() === "" ? undefined : (
      <ReplyQuote
        from={t("reply.quoteFrom", {
          name: displayName(answered.from),
          date: new Intl.DateTimeFormat(i18n.language, {
            weekday: "short",
            day: "numeric",
            month: "short",
          }).format(new Date(answered.sent_at)),
        })}
        text={answered.text.content.trim().slice(0, QUOTE_CHARS)}
      />
    );
  const to =
    recipient === undefined ? undefined : (
      <Text tone={elsewhere === undefined ? "secondary" : "danger"} size="sm">
        {elsewhere === undefined
          ? t("reply.to", { recipient: addressLine(recipient) })
          : t("reply.replyToElsewhere", {
              recipient: addressLine(elsewhere),
              sender: addressLine(answered?.from ?? elsewhere),
            })}
      </Text>
    );
  const problemLine =
    problem === undefined ? null : (
      <Banner
        tone={problem === "unavailable" ? "neutral" : "danger"}
        role={problem === "unavailable" ? undefined : "alert"}
        title={t(`reply.${problem}`, { name: person })}
      />
    );
  const writeOwn = (
    <Button
      variant={unavailable ? "primary" : "secondary"}
      icon="reply"
      onClick={() => {
        onWriteOwn();
      }}
    >
      {t("reply.writeOwn")}
    </Button>
  );

  const describe = (): string => {
    if (stage === "writing") {
      return t("reply.writing");
    }
    if (drafting) {
      return t("reply.draftDescription");
    }
    if (unavailable) {
      return t("reply.unavailable");
    }
    if (suggestions.isPending) {
      return t("reply.reading");
    }
    if (suggestions.isError) {
      return t("reply.suggestionsFailed");
    }
    return intents.length === 0 ? t("reply.noSuggestions") : t("reply.choose");
  };

  return (
    <Dialog
      open
      onClose={onClose}
      title={t("reply.title", { name: firstName(person) })}
      description={describe()}
      visor={
        stage === "writing" || (stage === "choose" && suggestions.isFetching) ? "thinking" : "idle"
      }
      result={result}
      onResultEnd={onClose}
      footer={
        drafting ? (
          <>
            <Button
              variant="primary"
              icon="send"
              shortcut="compose.send"
              disabled={busy || rewriting !== undefined || text.trim() === ""}
              onClick={() => {
                void send();
              }}
            >
              {t("reply.send")}
            </Button>
            <Button
              disabled={busy || rewriting !== undefined}
              onClick={() => {
                void openInComposer();
              }}
            >
              {t("reply.openInComposer")}
            </Button>
          </>
        ) : stage === "choose" ? (
          writeOwn
        ) : undefined
      }
    >
      {to}
      {quote}
      {stage === "choose" ? (
        <>
          {problemLine}
          {unavailable || intents.length === 0 ? null : (
            <DialogChoices
              label={t("reply.choices")}
              focusOnShow
              choices={intents.map((label, index) => ({ id: String(index), label, ai: true }))}
              onChoose={(id) => {
                const picked = intents[Number(id)];
                if (picked !== undefined) {
                  write(picked);
                }
              }}
            />
          )}
          {unavailable ? null : (
            <AskLine
              label={t("reply.own.label")}
              placeholder={t("reply.own.placeholder")}
              action={t("reply.own.action")}
              maxLength={MAX_ASK_CHARS}
              onSubmit={(own) => {
                write(own);
              }}
            />
          )}
        </>
      ) : null}
      {drafting ? (
        <>
          <ReplyDraft
            label={t("reply.draftLabel")}
            value={text}
            onChange={setText}
            busy={rewriting !== undefined}
            busyLabel={
              rewriting === "shorter"
                ? t("reply.shortening")
                : rewriting === "change"
                  ? t("reply.changing")
                  : undefined
            }
            tools={
              <>
                <Button
                  ai
                  disabled={busy || rewriting !== undefined || text.trim() === ""}
                  onClick={() => {
                    write(intent, { shorter: true });
                  }}
                >
                  {t("reply.shorter")}
                </Button>
                {resume === undefined ? (
                  <Button variant="ghost" disabled={busy} onClick={back}>
                    {t("reply.back")}
                  </Button>
                ) : null}
              </>
            }
          />
          <AskLine
            label={t("reply.change.label")}
            placeholder={t("reply.change.placeholder")}
            action={t("reply.change.action")}
            maxLength={MAX_ASK_CHARS}
            disabled={busy || rewriting !== undefined || text.trim() === ""}
            onSubmit={(change) => {
              write(intent, { change });
            }}
          />
          {problemLine}
        </>
      ) : null}
    </Dialog>
  );
}
