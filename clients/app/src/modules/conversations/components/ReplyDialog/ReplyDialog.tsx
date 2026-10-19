import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
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
  ThinkingTrace,
  useUndoToast,
  type DialogResult,
} from "~/lib/ui";

import {
  cancelSend,
  changedElsewhere,
  createDraft,
  deleteDraft,
  draftKeys,
  saveDraft,
  sendDraft,
  type Draft,
} from "../../api/drafts";
import { useConversation, useMessageWords, type MessageView } from "../../api/queries";
import { answeredPerson } from "../../api/person-notes";
import { streamReply, useReplySuggestions } from "../../api/replies";
import {
  addressLine,
  firstName,
  MAX_ASK_CHARS,
  quotePreview,
  quoteWords,
  undoWindow,
} from "../../compose/dialog";
import { displayName, replyQuote } from "../../compose/quote";
import { useDeleteDraft } from "../../compose/use-delete-draft";
import type { Translate } from "../../model";
import { AboutPerson } from "../PersonNotes/AboutPerson";

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

/** Who a reply to `message` goes to: the server sends it there (Reply-To first). */
function recipientOf(message: MessageView) {
  return message.direction === "outbound" ? message.to[0] : (message.reply_to[0] ?? message.from);
}

/**
 * Everyone a reply to `message` goes to, as the server fills a reply (its
 * Reply-To, else its sender; for a message the inbox sent, its To). The
 * dialog shows them all and saves exactly them, with no copies: a draft
 * holds only what the member sees here (campbooks-cloud#242).
 */
function recipientsOf(message: MessageView): Draft["to"] {
  let parties = [message.from];
  if (message.direction === "outbound") {
    parties = message.to;
  } else if (message.reply_to.length > 0) {
    parties = message.reply_to;
  }
  return parties.map((party) => ({ name: party.name ?? null, address: party.address }));
}

/**
 * Reply: Scout's likely replies for the newest message, a line to say it in
 * your words, and "Write my own". A pick asks Scout for the draft (only
 * then), shown editable with Shorter and Change…; Send saves it as a reply
 * draft and sends it with the usual undo window. Nothing is created before
 * Send or Open in composer, and Scout's text is only ever shown as text.
 * On the web the draft streams (campbooks-cloud#288): Scout's thinking
 * shows as it arrives, then the reply types in.
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
  const deleteWithUndo = useDeleteDraft(workspaceId);
  const detail = useConversation(workspaceId, conversationId);
  const messages = detail.data?.messages ?? [];
  // The newest message, as the reader's Reply answers it.
  const newest = messages.at(-1);
  // The message a draft is written for, pinned once one is asked for (or
  // handed back): mail arriving meanwhile never changes what Send answers
  // (campbooks-cloud#220). Until then the dialog follows the newest.
  const [pinned, setPinned] = useState<string | undefined>(resume?.draft.message_id ?? undefined);
  const base = pinned === undefined ? newest : messages.find((m) => m.id === pinned);
  // A newer message than the one the draft answers: said, with a way to
  // answer it instead.
  // Your own mail sent meanwhile is not something to answer.
  const newer =
    pinned !== undefined &&
    newest !== undefined &&
    newest.id !== pinned &&
    newest.direction === "inbound"
      ? newest
      : undefined;
  // Its words, asked for on their own when the conversation came without
  // them (`text_unread`).
  const unread = useMessageWords(workspaceId, conversationId, base);
  const answered =
    base?.text_unread === true && unread.data !== undefined
      ? { ...base, text: unread.data, text_unread: false }
      : base;
  // Who it really goes to, never only the sender's display name: a Reply-To
  // elsewhere is said out loud.
  const recipient = answered === undefined ? undefined : recipientOf(answered);
  const recipients = answered === undefined ? [] : recipientsOf(answered);
  const person = recipient === undefined ? (name ?? "") : displayName(recipient);
  const elsewhere =
    answered?.direction === "inbound" &&
    answered.reply_to[0] !== undefined &&
    answered.reply_to[0].address.toLowerCase() !== answered.from.address.toLowerCase()
      ? answered.reply_to[0]
      : undefined;
  const suggestions = useReplySuggestions(workspaceId, conversationId, answered?.id, {
    enabled: resume === undefined,
  });
  const intents = suggestions.data?.intents ?? [];

  const [stage, setStage] = useState<Stage>(resume === undefined ? "choose" : "draft");
  const [intent, setIntent] = useState(resume?.intent ?? "");
  const [text, setText] = useState(resume?.draft.body ?? "");
  // The draft this dialog created and saved: the only one it saves again.
  const [saved, setSaved] = useState<Draft | undefined>();
  // A draft the next one replaces (the one an Undo handed back, or one that
  // changed after it was saved): discarded once the new one exists.
  const replaced = useRef<Draft | undefined>(resume?.draft);
  // The draft an Undo handed back, until a fresh one replaces it: Delete
  // draft may throw it away.
  const [handedBack, setHandedBack] = useState<Draft | undefined>(resume?.draft);
  const [rewriting, setRewriting] = useState<"shorter" | "change" | undefined>();
  // Scout's thinking for the draft being asked for, as it arrives.
  const [thinking, setThinking] = useState("");
  // The reply's words are still arriving.
  const [streaming, setStreaming] = useState(false);
  const [problem, setProblem] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<DialogResult | undefined>();
  // Answers to a question no longer asked (Back, another pick) are dropped.
  const asking = useRef(0);
  // The request in flight, stopped by a new ask, Back, or closing.
  const inFlight = useRef<AbortController | undefined>(undefined);
  useEffect(() => {
    const request = inFlight;
    return () => {
      request.current?.abort();
    };
  }, []);
  // One send or save at a time, even for two quick ⌘↵.
  const working = useRef(false);

  const unavailable = problemCode(suggestions.error) === "reply_unavailable";
  const words = quoteWords(t);

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
      case "reply_timed_out":
      case "request_timed_out": {
        return "errors.timeout";
      }
      default: {
        return "errors.write";
      }
    }
  };

  const write = (
    asked: string,
    how?: { shorter?: boolean; change?: string },
    target: MessageView | undefined = answered,
  ) => {
    if (target === undefined) {
      return;
    }
    asking.current += 1;
    const ticket = asking.current;
    inFlight.current?.abort();
    const request = new AbortController();
    inFlight.current = request;
    const current = () => asking.current === ticket;
    const rewrite = how !== undefined;
    // A rewrite keeps the member's text until Scout's first word, and puts
    // it back if the rewrite fails.
    const before = text;
    let typing = false;
    setProblem(undefined);
    setThinking("");
    if (rewrite) {
      setRewriting(how.shorter === true ? "shorter" : "change");
    } else {
      setIntent(asked);
      setStage("writing");
      // Pinned from the moment it is asked for.
      setPinned(target.id);
    }
    void streamReply(
      workspaceId,
      conversationId,
      {
        message_id: target.id,
        intent: asked,
        // The likely replies shown, so "1 and 3" means something.
        suggestions: intents,
        ...(rewrite
          ? { draft: text, shorter: how.shorter === true, change: how.change ?? null }
          : {}),
      },
      {
        signal: request.signal,
        onThinking: (delta) => {
          if (current()) {
            setThinking((so) => so + delta);
          }
        },
        onText: (delta) => {
          if (!current()) {
            return;
          }
          if (typing) {
            setText((so) => so + delta);
            return;
          }
          typing = true;
          setText(delta);
          setStreaming(true);
          setStage("draft");
        },
      },
    )
      .then((written) => {
        if (!current()) {
          return;
        }
        setText(written);
        setStage("draft");
      })
      .catch((thrown: unknown) => {
        if (!current() || request.signal.aborted) {
          return;
        }
        setProblem(failure(thrown));
        // A failed first draft goes back to the choices (which follow the
        // newest message again); a failed rewrite keeps the text the member
        // had.
        if (rewrite) {
          setText(before);
        } else {
          setStage("choose");
          setPinned(undefined);
        }
      })
      .finally(() => {
        if (current()) {
          setRewriting(undefined);
          setStreaming(false);
        }
      });
  };

  const back = () => {
    asking.current += 1;
    inFlight.current?.abort();
    setRewriting(undefined);
    setStreaming(false);
    setThinking("");
    setProblem(undefined);
    setStage("choose");
    setPinned(undefined);
    // A draft already saved (a send that failed) is never reused for the
    // next pick, which may answer another message: the next save is a fresh
    // draft, and that one goes (campbooks-cloud#257).
    if (saved !== undefined) {
      replaced.current = saved;
      setSaved(undefined);
    }
  };

  /**
   * Answer the newer message instead: the draft written for the old one is
   * written again for it (or, with no intent to write from, its own text
   * now quotes the newer one). A draft already saved for the old one is
   * never reused: the next save is a fresh draft, and that one goes.
   */
  const answerNewer = () => {
    if (newer === undefined) {
      return;
    }
    if (saved !== undefined) {
      replaced.current = saved;
      setSaved(undefined);
    }
    if (intent.trim() === "") {
      setPinned(newer.id);
    } else {
      write(intent, undefined, newer);
    }
  };

  /**
   * The reply draft holding what is on screen (campbooks-cloud#242). Only a
   * draft this dialog created is saved again, with its token, the
   * recipients shown, its quoted text and no attachments. Any other (one
   * already in the drafts list, the one an Undo handed back, one that
   * changed after it was saved) is never reused: a fresh reply draft is
   * created, so nothing the dialog does not show (attachments, reply
   * headers, a subject someone else set) can go out. A draft it replaces is
   * discarded once the new one exists.
   */
  const keep = async (): Promise<Draft | undefined> => {
    if (answered === undefined) {
      return undefined;
    }
    // Only the quote the dialog shows: never words read after it said
    // they could not be, sent unseen.
    if (answered.text_unread === true) {
      throw new Error("the quote is not shown yet");
    }
    // The recipients shown, and no copies: what is saved is what is seen.
    const content = {
      body: text,
      quoted: replyQuote(answered, i18n.language, words),
      to: recipients,
      cc: [],
      bcc: [],
    };
    // Only a draft of the message answered is saved again.
    if (saved?.message_id === answered.id) {
      const again = await saveDraft(workspaceId, saved.id, {
        version: saved.version,
        content_token: saved.content_token ?? null,
        attachments: [],
        ...content,
      });
      setSaved(again);
      return again;
    }
    const fresh = await createDraft(workspaceId, {
      kind: "reply",
      message_id: answered.id,
      ...content,
    });
    setSaved(fresh);
    const old = replaced.current;
    replaced.current = undefined;
    setHandedBack(undefined);
    // Only one that is still editable, a draft or a send that failed: a
    // send on its way is taken back by its Undo alone, never discarded here
    // (campbooks-cloud#242).
    if (old?.state === "draft" || old?.state === "failed") {
      // Best effort: left behind, it is only a draft the composer shows whole.
      void deleteDraft(workspaceId, old.id).catch(() => {
        // Nothing to say: the reply itself is saved.
      });
    }
    return fresh;
  };

  const send = async () => {
    if (working.current || text.trim() === "") {
      return;
    }
    working.current = true;
    setBusy(true);
    setProblem(undefined);
    const handle = undo.pending({ label: t("reply.pending", { name: person }) });
    let kept: Draft | undefined;
    try {
      if (answered === undefined) {
        // Who it goes to is not on screen yet: nothing is sent unseen.
        throw new Error("nothing to answer yet");
      }
      const draft = await keep();
      if (draft === undefined) {
        throw new Error("nothing to answer yet");
      }
      kept = draft;
      // As just saved: the save's answer is what the dialog shows.
      const scheduled = await sendDraft(workspaceId, draft.id, {
        version: draft.version,
        contentToken: draft.content_token,
      });
      const window = undoWindow(scheduled.send_at);
      const asked = intent;
      // No send time: there is no window to take it back in, so no Undo
      // (it could only say "too late").
      if (window === undefined) {
        handle.done({ label: t("reply.sending", { name: person }) });
      } else {
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
      }
      void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      setResult({
        title: t("reply.result", { name: person }),
        detail: t("reply.resultDetail"),
      });
      onSent?.(scheduled);
    } catch (thrown) {
      if (changedElsewhere(thrown)) {
        // Changed after it was saved (saved since elsewhere, `draft_changed`,
        // refused by the save or by Send; or not what was shown,
        // `content_changed`): nothing sent. Send again writes what is shown
        // here into a fresh draft, and this one goes (campbooks-cloud#242).
        handle.fail({ label: t("reply.errors.changed") });
        setProblem("errors.changed");
        setSaved(undefined);
        replaced.current = kept ?? saved;
        void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      } else {
        handle.fail({ label: t("reply.errors.send", { name: person }) });
        setProblem("errors.send");
      }
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
      // The composer finds it at once: it reopens the reply draft of the
      // message the draft answers (`onOpenDraft` names it).
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

  // The draft that exists for this reply: the one saved here, else the one
  // an Undo handed back while it is still editable.
  const deletable =
    saved ??
    (handedBack?.state === "draft" || handedBack?.state === "failed" ? handedBack : undefined);

  /** Throw away the draft that exists (Undo writes it back as on screen) and leave. */
  const discard = async () => {
    if (working.current || deletable === undefined) {
      return;
    }
    working.current = true;
    setBusy(true);
    try {
      await deleteWithUndo({ ...deletable, body: text });
      onClose();
    } catch {
      setProblem("errors.delete");
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
    {
      enabled:
        drafting &&
        rewriting === undefined &&
        !streaming &&
        result === undefined &&
        answered?.text_unread !== true,
    },
  );

  // The message answered, the one the draft replies to.
  // Still reading its words, or they could not be read: said, never an
  // empty quote shown as if there were nothing.
  const quoteUnread = base?.text_unread === true && unread.isError && !unread.isFetching;
  const quoteReading = base?.text_unread === true && unread.data === undefined && !quoteUnread;
  // What the preview leaves out is said (campbooks-cloud#255).
  const preview = quotePreview(t, answered, i18n.language);
  const quote =
    preview === undefined ? undefined : (
      <ReplyQuote from={preview.from} text={preview.text} note={preview.note} />
    );
  const to =
    recipient === undefined ? undefined : (
      <Text tone={elsewhere === undefined ? "secondary" : "danger"} size="sm">
        {elsewhere === undefined
          ? t("reply.to", { recipient: recipients.map(addressLine).join(", ") })
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
  // Who Scout writes to, as the server reads them (the sender of mail
  // received, the first recipient of mail sent): what the member may tell
  // Scout about them. Not while Scout can't write replies here; a sender
  // that fails its checks gets no note on the server, whatever this shows.
  const about = answered === undefined ? undefined : answeredPerson(answered);
  const aboutLine =
    about === undefined || unavailable || problem === "unavailable" ? null : (
      <AboutPerson
        workspaceId={workspaceId}
        address={about.address}
        name={firstName(about.name ?? about.address)}
        onRewrite={
          stage === "draft" && intent.trim() !== ""
            ? () => {
                write(intent);
              }
            : undefined
        }
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

  // What Scout is doing to the draft: typing it in, or rewriting it.
  let busyLabel: string | undefined;
  if (streaming) {
    busyLabel = t("reply.typing");
  } else if (rewriting === "shorter") {
    busyLabel = t("reply.shortening");
  } else if (rewriting === "change") {
    busyLabel = t("reply.changing");
  }

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
              disabled={
                busy ||
                rewriting !== undefined ||
                streaming ||
                text.trim() === "" ||
                answered?.text_unread === true
              }
              onClick={() => {
                void send();
              }}
            >
              {t("reply.send")}
            </Button>
            <Button
              disabled={
                busy || rewriting !== undefined || streaming || answered?.text_unread === true
              }
              onClick={() => {
                void openInComposer();
              }}
            >
              {t("reply.openInComposer")}
            </Button>
            {deletable === undefined ? null : (
              <Button
                disabled={busy || rewriting !== undefined || streaming}
                onClick={() => {
                  void discard();
                }}
              >
                {t("compose.deleteDraft")}
              </Button>
            )}
          </>
        ) : stage === "choose" ? (
          writeOwn
        ) : undefined
      }
    >
      {to}
      {newer === undefined || stage === "choose" ? null : (
        <Banner
          icon="mail"
          title={t("reply.newer", { name: displayName(newer.from) })}
          action={
            <Button
              variant="ghost"
              disabled={busy || stage === "writing" || rewriting !== undefined}
              onClick={answerNewer}
            >
              {t("reply.answerNewer")}
            </Button>
          }
        />
      )}
      {quoteUnread ? (
        <Banner
          tone="danger"
          role="alert"
          title={t("reply.quoteUnread")}
          action={
            <Button
              onClick={() => {
                void unread.refetch();
              }}
            >
              {t("error.retry")}
            </Button>
          }
        />
      ) : null}
      {quoteReading ? (
        <div role="status">
          <Text tone="secondary" size="sm">
            {t("reply.readingQuote")}
          </Text>
        </div>
      ) : null}
      {quoteUnread || quoteReading ? null : quote}
      {aboutLine}
      {stage === "writing" || (drafting && thinking !== "") ? (
        <ThinkingTrace
          label={t("reply.thinking")}
          text={thinking}
          live={stage === "writing" || (rewriting !== undefined && !streaming)}
          summary={t("reply.thoughts")}
        />
      ) : null}
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
            busy={rewriting !== undefined || streaming}
            busyLabel={busyLabel}
            tools={
              <>
                <Button
                  ai
                  disabled={busy || rewriting !== undefined || streaming || text.trim() === ""}
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
            disabled={busy || rewriting !== undefined || streaming || text.trim() === ""}
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
