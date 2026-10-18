import { useTranslation } from "react-i18next";

import { MessageComposer, useUndoToast } from "~/lib/ui";

import type { Draft } from "../../api/drafts";
import type { Translate } from "../../model";
import { addressLine, displayName } from "../../compose/quote";
import { useDraftEditor } from "../../compose/use-draft-editor";

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
}

const KNOWN_ERRORS = new Set([
  "not_saved",
  "draft_changed",
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

/** A draft in the message composer: saved as you type, sent with an undo window. */
export function DraftComposer({
  workspaceId,
  draft,
  title,
  withSubject,
  focus,
  onClose,
  onDone,
  onUndone,
}: DraftComposerProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undoToast = useUndoToast();
  const editor = useDraftEditor(workspaceId, draft);
  const { fields, update } = editor;

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
    await editor.discard();
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
      attachments={editor.shown.attachments.filter((a) => fields.attachments.includes(a.id))}
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
    />
  );
}
