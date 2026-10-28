import { useState } from "react";
import { useTranslation } from "react-i18next";

import { MessageComposer, sendTimeLabel, useUndoToast } from "~/lib/ui";

import type { Draft } from "../../api/drafts";
import type { Translate } from "../../model";
import {
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENTS,
  refusalOfCode,
  screen,
  type RefusalReason,
} from "../../compose/attachment-rules";
import { addressLine, displayName } from "../../compose/quote";
import { useDraftEditor } from "../../compose/use-draft-editor";
import { useRecipientSuggestions } from "../../compose/use-recipient-suggestions";

export interface DraftComposerProps {
  workspaceId: string;
  draft: Draft;
  /** "Reply to Maya Okafor", "Forward", "New message". */
  title: string;
  /** Show the subject (new messages and forwards). */
  withSubject: boolean;
  /** Where the cursor starts. */
  focus: "to" | "body";
  /** Closed, keeping the draft (Esc). */
  onClose: () => void;
  /** Sent (scheduled, with Undo in a toast) or discarded: the composer goes. */
  onDone: () => void;
  /** The send was taken back: show the draft again. */
  onUndone?: (() => void) | undefined;
  /** Said beside the quoted history (that it holds only the start of a long email). */
  quotedNote?: string | undefined;
}

const KNOWN_ERRORS = new Set([
  "not_saved",
  "content_changed",
  "no_recipients",
  "invalid_address",
  "attachments_too_large",
  "send_at_invalid",
  "draft_not_editable",
  "not_connected",
  "needs_reauth",
  "rejected",
  "too_large",
  "unavailable",
  "unknown_outcome",
]);

/**
 * A draft in the message composer: saved as you type, sent with an undo
 * window, with people suggested in To, Cc and Bcc as you type.
 */
export function DraftComposer({
  workspaceId,
  draft,
  title,
  withSubject,
  focus,
  onClose,
  onDone,
  onUndone,
  quotedNote,
}: DraftComposerProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undoToast = useUndoToast();
  const editor = useDraftEditor(workspaceId, draft);
  const { fields, update } = editor;
  // To, Cc and Bcc suggest the people you exchange mail with as you type.
  const suggest = useRecipientSuggestions(workspaceId, fields);

  const kept = editor.shown.attachments.filter((a) => fields.attachments.includes(a.id));

  // Why the files last added were not attached, said in the composer beside
  // them (a toast is one line and would cut the reason at 375px).
  const [refusals, setRefusals] = useState<string[]>([]);

  /** Say, naming the file, why it was not attached. */
  function refuse(file: File, reason: RefusalReason) {
    const said = t(`compose.attach.refused.${reason}`, {
      name: file.name,
      max: Math.round(MAX_ATTACHMENT_BYTES / (1024 * 1024)),
      files: MAX_ATTACHMENTS,
    });
    setRefusals((so) => [...so, said]);
  }

  /** Files from the picker, a drop or a paste: refuse what cannot go, upload the rest. */
  function attach(files: File[]) {
    setRefusals([]);
    const { accepted, refused } = screen(files, {
      bytes:
        kept.reduce((sum, a) => sum + a.size, 0) +
        editor.uploading.reduce((sum, u) => sum + u.size, 0),
      count: kept.length + editor.uploading.length,
    });
    for (const { file, reason } of refused) {
      refuse(file, reason);
    }
    if (accepted.length === 0) {
      return;
    }
    void editor.attach(accepted).then((failed) => {
      for (const { file, code } of failed) {
        const reason = refusalOfCode(code);
        if (reason !== undefined) {
          refuse(file, reason);
        } else if (code !== "content_changed" && code !== "draft_changed") {
          // A draft changed elsewhere has its notice already.
          undoToast.notify(t(`compose.errors.${KNOWN_ERRORS.has(code) ? code : "unknown"}`));
        }
      }
    });
  }

  /** Send now (`sendAt` absent) or at `sendAt`, then close with an Undo. */
  async function send(sendAt?: Date) {
    const scheduled = await editor.send(sendAt);
    if (scheduled === undefined) {
      return;
    }
    const first = scheduled.to[0] ?? scheduled.cc[0] ?? scheduled.bcc[0];
    // Undo stays on offer for the whole window the server gives; for a send
    // later, a short while (the Drafts list takes it back after that).
    const window =
      scheduled.send_at === null || scheduled.send_at === undefined || sendAt !== undefined
        ? undefined
        : Math.max(1000, new Date(scheduled.send_at).getTime() - Date.now());
    undoToast.push({
      label:
        sendAt === undefined
          ? t("compose.sending", { name: first === undefined ? "" : displayName(first) })
          : t("compose.scheduled", { time: sendTimeLabel(sendAt, i18n.language) }),
      durationMs: window,
      undo: () => {
        void editor.undo().then((inTime) => {
          if (inTime) {
            onUndone?.();
          } else {
            undoToast.notify(t("compose.tooLate"));
          }
        });
      },
    });
    onDone();
  }

  async function discard() {
    try {
      await editor.discard();
    } catch {
      // Nothing changed: the draft is still here.
      undoToast.notify(t("compose.errors.unknown"));
      return;
    }
    onDone();
  }

  const error =
    editor.error === undefined
      ? refusals.length === 0
        ? undefined
        : refusals.join(" ")
      : t(`compose.errors.${KNOWN_ERRORS.has(editor.error) ? editor.error : "unknown"}`);

  return (
    <MessageComposer
      title={title}
      from={addressLine(editor.shown.from)}
      to={fields.to}
      onToChange={(to) => {
        update({ to });
      }}
      cc={fields.cc}
      onCcChange={(cc) => {
        update({ cc });
      }}
      bcc={fields.bcc}
      onBccChange={(bcc) => {
        update({ bcc });
      }}
      subject={withSubject ? fields.subject : undefined}
      onSubjectChange={(subject) => {
        update({ subject });
      }}
      body={fields.body}
      onBodyChange={(body) => {
        update({ body });
      }}
      quoted={fields.quoted}
      onQuotedChange={(quoted) => {
        update({ quoted });
      }}
      quotedNote={quotedNote}
      attachments={kept}
      onAttach={attach}
      uploading={editor.uploading.map((u) => ({ key: u.key, filename: u.name, size: u.size }))}
      attachmentBudget={MAX_ATTACHMENT_BYTES}
      onRemoveAttachment={(id) => {
        update({ attachments: fields.attachments.filter((kept) => kept !== id) });
      }}
      status={t(`compose.saveState.${editor.saveState}`)}
      error={error}
      sending={editor.sending}
      focus={focus}
      onSend={() => {
        void send();
      }}
      onSendLater={(at) => {
        void send(at);
      }}
      onClose={() => {
        void editor.flush();
        onClose();
      }}
      onDiscard={() => {
        void discard();
      }}
      suggest={suggest}
    />
  );
}
