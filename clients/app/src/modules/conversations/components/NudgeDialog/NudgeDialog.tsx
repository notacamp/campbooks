import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { conversationRef } from "~/lib/scout-chat";
import { useShortcut } from "~/lib/shortcuts";
import {
  AskLine,
  Banner,
  Button,
  Dialog,
  ReplyDraft,
  ReplyQuote,
  Text,
  useUndoToast,
  type DialogResult,
} from "~/lib/ui";

import { AskScoutIcon } from "../AskScout/AskScoutIcon";
import {
  cancelSend,
  changedElsewhere,
  createDraft,
  deleteDraft,
  draftKeys,
  saveDraft,
  sendDraft,
  useConversationDrafts,
  type Draft,
} from "../../api/drafts";
import {
  useConversation,
  useInboxes,
  useMessageWords,
  type MessageView,
  type NeedView,
} from "../../api/queries";
import { useNudgeText, writeNudge } from "../../api/replies";
import {
  addressLine,
  firstName,
  literalLinks,
  MAX_ASK_CHARS,
  quotePreview,
  quoteWords,
  shownLinks,
  undoWindow,
} from "../../compose/dialog";
import { displayName, replyQuote } from "../../compose/quote";
import { useDeleteDraft } from "../../compose/use-delete-draft";
import type { Translate } from "../../model";

/** A nudge whose send was taken back in time, to pick up again. */
export interface NudgeResume {
  draft: Draft;
}

export interface NudgeDialogProps {
  workspaceId: string;
  /** The conversation that waits. */
  conversationId: string;
  /** Its wait: the message it comes from, who it is on, how many nudges went. */
  need: NeedView;
  /** Start on this draft (its send was undone). */
  resume?: NudgeResume | undefined;
  /** Esc, a click outside, or the result has shown: nothing more to do here. */
  onClose: () => void;
  /** "Open in composer" and "Write my own": the nudge draft, addressed, in the composer. */
  onOpenDraft: (draft: Draft) => void;
  /** Sent: it leaves when the undo window ends. */
  onSent?: ((draft: Draft) => void) | undefined;
  /** The send was taken back in time: offer the draft again. */
  onUndone?: ((resume: NudgeResume) => void) | undefined;
}

type Address = Draft["to"][number];

const same = (a: Address, b: Address) =>
  a.address.trim().toLowerCase() === b.address.trim().toLowerCase();

/**
 * Who a nudge goes to, as the server addresses the `nudge` draft kind: the
 * awaited person in To, and everyone else a reply all to `message` reaches
 * in Cc (its Reply-To or sender, its To and Cc), never the inbox itself and
 * never twice.
 */
function nudgeRecipients(
  message: MessageView,
  awaited: Address,
  inbox: string,
): { to: Address[]; cc: Address[] } | undefined {
  const ours = (party: Address) => same(party, { address: inbox });
  // Never a nudge to the inbox itself: nothing to wait for.
  if (ours(awaited)) {
    return undefined;
  }
  const parties =
    message.direction === "outbound"
      ? message.to
      : [...(message.reply_to.length > 0 ? message.reply_to : [message.from]), ...message.to];
  const cc: Address[] = [];
  for (const party of [...parties, ...message.cc]) {
    const address = party.address.trim();
    const name = party.name?.trim();
    if (
      address === "" ||
      ours(party) ||
      same(party, awaited) ||
      cc.some((kept) => same(kept, party))
    ) {
      continue;
    }
    cc.push({ address, name: name === undefined || name === "" ? null : name });
  }
  return { to: [awaited], cc };
}

/** Whether `draft` goes to exactly the people `shown` (case aside). */
function sameAddressing(draft: Draft, shown: { to: Address[]; cc: Address[] }): boolean {
  const key = (parties: Address[]) =>
    parties
      .map((party) => party.address.trim().toLowerCase())
      .toSorted()
      .join(",");
  return key(draft.to) === key(shown.to) && key(draft.cc) === key(shown.cc);
}

/** People as their addresses read, each isolated from the line's text direction. */
function People({ parties }: { parties: Address[] }) {
  return (
    <>
      {parties.map((party, index) => (
        <span key={party.address}>
          {index === 0 ? null : ", "}
          <bdi>{addressLine(party)}</bdi>
        </span>
      ))}
    </>
  );
}

/** A draft the member can still change. */
const editable = (draft: Draft | undefined) =>
  draft?.state === "draft" || draft?.state === "failed";

/**
 * Nudge: Scout's few lines to the person a conversation waits on
 * (campbooks-cloud#293), in the same thread, copying the others as a reply
 * all would. Scout writes when the dialog opens; the text is editable, with
 * Shorter and Change…; Send saves it as a `nudge` draft and sends it with the
 * usual undo window. Nothing is created before Send, Open in composer or
 * Write my own, and Scout's text is only ever shown as text.
 */
export function NudgeDialog({
  workspaceId,
  conversationId,
  need,
  resume,
  onClose,
  onOpenDraft,
  onSent,
  onUndone,
}: NudgeDialogProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const queryClient = useQueryClient();
  const undo = useUndoToast();
  const deleteWithUndo = useDeleteDraft(workspaceId);
  const detail = useConversation(workspaceId, conversationId);
  const drafts = useConversationDrafts(workspaceId, conversationId);
  const inboxes = useInboxes(workspaceId).data;
  const inbox = inboxes?.find((each) => each.id === detail.data?.conversation.inbox_id);
  // The message the wait comes from: the nudge answers it.
  const waited = detail.data?.messages.find((m) => m.id === need.message_id);
  // Its words, asked for on their own when the conversation came without
  // them (`text_unread`).
  const unread = useMessageWords(workspaceId, conversationId, waited);
  const answered =
    waited?.text_unread === true && unread.data !== undefined
      ? { ...waited, text: unread.data, text_unread: false }
      : waited;
  const awaited = need.waiting_on ?? undefined;
  const person = awaited === undefined ? "" : displayName(awaited);
  const first = firstName(person);
  // The draft this dialog created and saved: the only one it saves again.
  const [saved, setSaved] = useState<Draft | undefined>();
  // Waiting on the inbox's own address (mail it sent itself): nobody to nudge.
  const ownAddress =
    awaited !== undefined && inbox !== undefined && same(awaited, { address: inbox.address });
  // Who it goes to: as the server addressed the draft once one exists, else
  // as the server will (shown before anything is created).
  const recipients =
    saved === undefined
      ? answered === undefined || awaited === undefined || inbox === undefined
        ? undefined
        : nudgeRecipients(answered, awaited, inbox.address)
      : { to: saved.to, cc: saved.cc };

  const scout = useNudgeText(
    workspaceId,
    conversationId,
    { messageId: need.message_id, nudges: need.nudges ?? 0 },
    { enabled: resume === undefined },
  );
  // What the member has on screen once they (or a rewrite) changed it.
  const [edited, setEdited] = useState<string | undefined>(
    resume === undefined ? undefined : shownLinks(resume.draft.body),
  );
  const text = edited ?? scout.data ?? "";
  // A draft the next one replaces (the one an Undo handed back, or one that
  // changed after it was saved): discarded once the new one exists.
  const replaced = useRef<Draft | undefined>(resume?.draft);
  const [handedBack, setHandedBack] = useState<Draft | undefined>(resume?.draft);
  const [rewriting, setRewriting] = useState<"shorter" | "change" | undefined>();
  const [problem, setProblem] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<DialogResult | undefined>();
  // Answers to a rewrite no longer asked for are dropped.
  const asking = useRef(0);
  // One send or save at a time, even for two quick ⌘↵.
  const working = useRef(false);
  const words = quoteWords(t);

  // Scout's first text failed: why, from the server's code.
  const firstError: unknown = resume === undefined && scout.isError ? scout.error : undefined;
  const firstFailure = firstError === undefined ? undefined : problemCode(firstError);
  const unavailable = firstFailure === "reply_unavailable";
  const notWaiting = firstFailure === "not_waiting" || problem === "notWaiting" || ownAddress;
  const writing = resume === undefined && scout.isPending;
  const drafting = !writing && (resume !== undefined || scout.data !== undefined) && !notWaiting;

  const failure = (thrown: unknown): string => {
    switch (problemCode(thrown)) {
      case "not_waiting": {
        return "notWaiting";
      }
      case "reply_timed_out": {
        return "errors.timeout";
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
  // Scout's first text failed (not "no model here"): said, with a way to ask again.
  const firstProblem = firstError === undefined || unavailable ? undefined : failure(firstError);

  const rewrite = (how: { shorter?: boolean; change?: string }) => {
    asking.current += 1;
    const ticket = asking.current;
    setProblem(undefined);
    setRewriting(how.shorter === true ? "shorter" : "change");
    void writeNudge(workspaceId, conversationId, {
      draft: text,
      shorter: how.shorter === true,
      change: how.change ?? null,
    })
      .then((written) => {
        if (asking.current === ticket) {
          setEdited(written);
        }
      })
      .catch((thrown: unknown) => {
        // The member's text stays as it was.
        if (asking.current === ticket) {
          setProblem(failure(thrown));
        }
      })
      .finally(() => {
        if (asking.current === ticket) {
          setRewriting(undefined);
        }
      });
  };

  /**
   * The nudge draft holding what is on screen (campbooks-cloud#242): only a
   * draft this dialog created is saved again, with its token, the people
   * shown, its quoted text and no attachments. Any other is never reused,
   * so nothing the dialog does not show can go out; a draft it replaces is
   * discarded once the new one exists.
   */
  const keep = async (body: string): Promise<Draft> => {
    // Who it goes to and the quote are on screen before anything is saved.
    if (answered === undefined || recipients === undefined || answered.text_unread === true) {
      throw new Error("not shown yet");
    }
    // No To or Cc: the server addresses a nudge, and keeps its addressing
    // when the draft is saved again.
    const content = {
      body: literalLinks(body),
      quoted: replyQuote(answered, i18n.language, words),
      bcc: [],
    };
    if (saved !== undefined) {
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
      kind: "nudge",
      message_id: answered.id,
      ...content,
    });
    setSaved(fresh);
    const old = replaced.current;
    replaced.current = undefined;
    setHandedBack(undefined);
    // Only one still editable: a send on its way is taken back by its Undo alone.
    if (editable(old) && old !== undefined) {
      void deleteDraft(workspaceId, old.id).catch(() => {
        // Left behind, it is only a draft the composer shows whole.
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
    const handle = undo.pending({ label: t("reply.pending", { name: first }) });
    let kept: Draft | undefined;
    try {
      const shown = recipients;
      const draft = await keep(text);
      kept = draft;
      // Addressed otherwise than shown (the people on the email changed):
      // nothing sent; the dialog now shows where it goes.
      if (shown === undefined || !sameAddressing(draft, shown)) {
        handle.fail({ label: t("nudge.errors.addressed") });
        setProblem("addressed");
        return;
      }
      // As just saved: the save's answer is what the dialog shows.
      const scheduled = await sendDraft(workspaceId, draft.id, {
        version: draft.version,
        contentToken: draft.content_token,
      });
      const window = undoWindow(scheduled.send_at);
      if (window === undefined) {
        handle.done({ label: t("nudge.sent", { name: first }) });
      } else {
        handle.done({
          label: t("nudge.sent", { name: first }),
          durationMs: window,
          undo: () => {
            void cancelSend(workspaceId, scheduled.id)
              .then((back) => {
                undo.notify(t("nudge.undone"));
                void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
                onUndone?.({ draft: back });
              })
              .catch(() => {
                undo.notify(t("nudge.tooLate"));
              });
          },
        });
      }
      void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      setResult({
        title: t("reply.result", { name: first }),
        detail: t("reply.resultDetail"),
      });
      onSent?.(scheduled);
    } catch (thrown) {
      if (changedElsewhere(thrown)) {
        // Changed after it was saved: nothing sent. Send again writes what
        // is shown here into a fresh draft, and this one goes.
        handle.fail({ label: t("reply.errors.changed") });
        setProblem("errors.changed");
        setSaved(undefined);
        replaced.current = kept ?? saved;
        void queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
      } else if (problemCode(thrown) === "not_waiting") {
        handle.fail({ label: t("nudge.errors.notWaiting", { name: person }) });
        setProblem("notWaiting");
      } else {
        handle.fail({ label: t("reply.errors.send", { name: person }) });
        setProblem("errors.send");
      }
    } finally {
      working.current = false;
      setBusy(false);
    }
  };

  /** Hand `draft` to the composer, which finds it at once. */
  const open = (draft: Draft) => {
    queryClient.setQueryData<Draft[]>(
      draftKeys.forConversation(workspaceId, conversationId),
      (list) => [draft, ...(list ?? []).filter((d) => d.id !== draft.id)],
    );
    onOpenDraft(draft);
  };

  const openInComposer = async () => {
    if (working.current) {
      return;
    }
    working.current = true;
    setBusy(true);
    try {
      open(await keep(text));
    } catch {
      setProblem("errors.open");
    } finally {
      working.current = false;
      setBusy(false);
    }
  };

  /**
   * Write my own: the nudge draft the member already started for this wait,
   * as they left it (never written over), else a fresh one, addressed and
   * empty.
   */
  const writeOwn = async () => {
    if (working.current) {
      return;
    }
    const started = (drafts.data ?? []).find(
      (draft) => draft.kind === "nudge" && draft.message_id === need.message_id && editable(draft),
    );
    if (started !== undefined) {
      open(started);
      return;
    }
    working.current = true;
    setBusy(true);
    try {
      open(await keep(""));
    } catch (thrown) {
      setProblem(problemCode(thrown) === "not_waiting" ? "notWaiting" : "errors.open");
    } finally {
      working.current = false;
      setBusy(false);
    }
  };

  // The draft that exists for this nudge: the one saved here, else the one
  // an Undo handed back while it is still editable.
  const deletable = saved ?? (editable(handedBack) ? handedBack : undefined);

  /** Throw away the draft that exists (Undo writes it back as on screen) and leave. */
  const discard = async () => {
    if (working.current || deletable === undefined) {
      return;
    }
    working.current = true;
    setBusy(true);
    try {
      await deleteWithUndo({ ...deletable, body: literalLinks(text) });
      onClose();
    } catch {
      setProblem("errors.delete");
    } finally {
      working.current = false;
      setBusy(false);
    }
  };

  const quoteShown = answered?.text_unread !== true && recipients !== undefined;
  useShortcut(
    "compose.send",
    () => {
      void send();
    },
    { enabled: drafting && rewriting === undefined && result === undefined && quoteShown },
  );

  const quoteUnread = waited?.text_unread === true && unread.isError && !unread.isFetching;
  const quoteReading = waited?.text_unread === true && unread.data === undefined && !quoteUnread;
  const preview = quotePreview(t, answered, i18n.language);
  const shownProblem = problem ?? (ownAddress ? "ownAddress" : firstProblem);
  const calm = shownProblem === "notWaiting" || shownProblem === "ownAddress";
  const problemLine =
    shownProblem === undefined ? null : (
      <Banner
        tone={calm ? "neutral" : "danger"}
        role={calm ? undefined : "alert"}
        title={
          calm || shownProblem === "addressed"
            ? t(`nudge.errors.${shownProblem}`, { name: person })
            : t(`reply.${shownProblem}`, { name: person })
        }
        action={
          problem === undefined && firstProblem !== undefined && !notWaiting ? (
            <Button
              onClick={() => {
                void scout.refetch();
              }}
            >
              {t("error.retry")}
            </Button>
          ) : undefined
        }
      />
    );
  const writeOwnButton = (
    <Button
      variant={unavailable ? "primary" : drafting ? "ghost" : "secondary"}
      icon="reply"
      disabled={busy || !quoteShown}
      onClick={() => {
        void writeOwn();
      }}
    >
      {t("reply.writeOwn")}
    </Button>
  );

  const describe = (): string => {
    if (writing) {
      return t("nudge.writing");
    }
    if (unavailable) {
      return t("nudge.unavailable");
    }
    return t("nudge.description", { name: first });
  };

  return (
    <Dialog
      open
      onClose={onClose}
      title={t("nudge.title", { name: first })}
      description={describe()}
      visor={writing ? "thinking" : "idle"}
      result={result}
      onResultEnd={onClose}
      headerAction={
        <AskScoutIcon about={conversationRef(conversationId)} label={t("nudge.askScout")} />
      }
      footer={
        drafting ? (
          <>
            <Button
              variant="primary"
              icon="send"
              shortcut="compose.send"
              disabled={busy || rewriting !== undefined || text.trim() === "" || !quoteShown}
              onClick={() => {
                void send();
              }}
            >
              {t("reply.send")}
            </Button>
            <Button
              disabled={busy || rewriting !== undefined || !quoteShown}
              onClick={() => {
                void openInComposer();
              }}
            >
              {t("reply.openInComposer")}
            </Button>
            {writeOwnButton}
            {deletable === undefined ? null : (
              <Button
                disabled={busy || rewriting !== undefined}
                onClick={() => {
                  void discard();
                }}
              >
                {t("compose.deleteDraft")}
              </Button>
            )}
          </>
        ) : writing || notWaiting ? undefined : (
          writeOwnButton
        )
      }
    >
      {recipients === undefined ? null : (
        <div>
          <Text tone="secondary" size="sm">
            {t("nudge.to")} <People parties={recipients.to} />
          </Text>
          {recipients.cc.length === 0 ? null : (
            <Text tone="secondary" size="sm">
              {t("nudge.copying")} <People parties={recipients.cc} />
            </Text>
          )}
        </div>
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
      {quoteUnread || quoteReading || preview === undefined ? null : (
        <ReplyQuote from={preview.from} text={preview.text} note={preview.note} />
      )}
      {drafting ? (
        <>
          <ReplyDraft
            label={t("nudge.draftLabel")}
            value={text}
            onChange={setEdited}
            busy={rewriting !== undefined}
            busyLabel={
              rewriting === "shorter"
                ? t("reply.shortening")
                : rewriting === "change"
                  ? t("reply.changing")
                  : undefined
            }
            tools={
              <Button
                ai
                disabled={busy || rewriting !== undefined || text.trim() === ""}
                onClick={() => {
                  rewrite({ shorter: true });
                }}
              >
                {t("reply.shorter")}
              </Button>
            }
          />
          <AskLine
            label={t("reply.change.label")}
            placeholder={t("reply.change.placeholder")}
            action={t("reply.change.action")}
            maxLength={MAX_ASK_CHARS}
            disabled={busy || rewriting !== undefined || text.trim() === ""}
            onSubmit={(change) => {
              rewrite({ change });
            }}
          />
        </>
      ) : null}
      {problemLine}
    </Dialog>
  );
}
