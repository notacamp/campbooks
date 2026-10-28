import { useTranslation } from "react-i18next";

import { MessageComposer, useUndoToast } from "~/lib/ui";

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
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undoToast = useUndoToast();
  const editor = useDraftEditor(workspaceId, draft);
  const { fields, update } = editor;
  // To, Cc and Bcc suggest the people you exchange mail with as you type.
  const suggest = useRecipientSuggestions(workspaceId, fields);

  const kept = editor.shown.attachments.filter((a) => fields.attachments.includes(a.id));

  /** Say, naming the file, why it was not attached. */
  function refuse(file: File, reason: RefusalReason) {
    undoToast.notify(
      t(`compose.attach.refused.${reason}`, {
        name: file.name,
        max: Math.round(MAX_ATTACHMENT_BYTES / (1024 * 1024)),
        files: MAX_ATTACHMENTS,
      }),
    );
  }

  /** Files from the picker, a drop or a paste: refuse what cannot go, upload the rest. */
  function attach(files: File[]) {
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

  async function send() {
    const scheduled = await editor.send();
    if (scheduled === undefined) {
      return;
    }
    const first = scheduled.to[0] ?? scheduled.cc[0] ?? scheduled.bcc[0];
    // Undo stays on offer for the whole window the server gives.
    const window =
      scheduled.send_at === null || scheduled.send_at === undefined
        ? undefined
        : Math.max(1000, new Date(scheduled.send_at).getTime() - Date.now());
    undoToast.push({
      label: t("compose.sending", { name: first === undefined ? "" : displayName(first) }),
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
      ? undefined
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
