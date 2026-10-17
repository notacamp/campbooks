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
  useUndoToast,
  type DialogResult,
} from "~/lib/ui";

import {
  cancelSend,
  createDraft,
  draftKeys,
  saveDraft,
  sendDraft,
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

/** Longest quote shown from the message answered, in characters. */
const QUOTE_CHARS = 280;

/** The person a reply to `message` goes to. */
function personOf(message: MessageView): string {
  const to =
    message.direction === "outbound" ? message.to[0] : (message.reply_to[0] ?? message.from);
  return to === undefined ? "" : displayName(to);
}

/** The first name, for the title ("Reply to Marta"). */
function firstName(name: string): string {
  return name.includes("@") ? name : (name.split(/\s+/u)[0] ?? name);
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
  const theirs = messages.findLast((m) => m.direction === "inbound") ?? answered;
  const person = name ?? (answered === undefined ? "" : personOf(answered));
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

  /** The reply draft holding what is on screen: created once, then saved. */
  const keep = async (): Promise<Draft | undefined> => {
    if (answered === undefined) {
      return undefined;
    }
    const draft =
      saved === undefined
        ? await createDraft(workspaceId, {
            kind: "reply",
            message_id: answered.id,
            body: text,
            quoted: replyQuote(answered, i18n.language, words),
          })
        : await saveDraft(workspaceId, saved.id, { version: saved.version, body: text });
    setSaved(draft);
    queryClient.setQueryData<Draft[]>(
      draftKeys.forConversation(workspaceId, conversationId),
      (list) => [draft, ...(list ?? []).filter((d) => d.id !== draft.id)],
    );
    return draft;
  };

  const send = async () => {
    if (busy || text.trim() === "" || answered === undefined) {
      return;
    }
    setBusy(true);
    setProblem(undefined);
    const handle = undo.pending({ label: t("reply.pending", { name: person }) });
    try {
      const draft = await keep();
      if (draft === undefined) {
        return;
      }
      const scheduled = await sendDraft(workspaceId, draft.id, draft.version);
      const window =
        scheduled.send_at === null || scheduled.send_at === undefined
          ? undefined
          : Math.max(1000, new Date(scheduled.send_at).getTime() - Date.now());
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
      setBusy(false);
    }
  };

  const openInComposer = async () => {
    if (busy) {
      return;
    }
    setBusy(true);
    try {
      const draft = await keep();
      if (draft !== undefined) {
        onOpenDraft(draft);
      }
    } catch {
      setProblem("errors.open");
    } finally {
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
  const quote =
    theirs?.text?.content === undefined || theirs.text.content.trim() === "" ? undefined : (
      <ReplyQuote
        from={t("reply.quoteFrom", {
          name: displayName(theirs.from),
          date: new Intl.DateTimeFormat(i18n.language, {
            weekday: "short",
            day: "numeric",
            month: "short",
          }).format(new Date(theirs.sent_at)),
        })}
        text={theirs.text.content.trim().slice(0, QUOTE_CHARS)}
      />
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
                  disabled={busy || text.trim() === ""}
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
