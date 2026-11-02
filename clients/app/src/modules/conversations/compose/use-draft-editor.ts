import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";

import { problemCode } from "~/lib/api";

import {
  cancelSend,
  changedElsewhere,
  draftKeys,
  fetchDraft,
  saveDraft,
  sendDraft,
  uploadAttachment,
  type Draft,
  type DraftChanges,
} from "../api/drafts";
import { useDeleteDraft } from "./use-delete-draft";

/** What the member edits. */
export interface DraftFields {
  to: Draft["to"];
  cc: Draft["cc"];
  bcc: Draft["bcc"];
  subject: string;
  body: string;
  quoted: string;
  attachments: string[];
}

export type SaveState = "saved" | "saving" | "unsaved" | "failed";

/** A file the server did not take: why, as the problem code (`attachments_too_large`, ...). */
export interface RefusedFile {
  file: File;
  code: string;
}

/** A file being uploaded. */
export interface UploadingFile {
  key: number;
  name: string;
  size: number;
}

/** Saves are this far apart while the member types. */
const AUTOSAVE_MS = 800;

function fieldsOf(draft: Draft): DraftFields {
  return {
    to: draft.to,
    cc: draft.cc,
    bcc: draft.bcc,
    subject: draft.subject,
    body: draft.body,
    quoted: draft.quoted,
    attachments: draft.attachments.map((a) => a.id),
  };
}

/** What the composer shows of a draft but does not edit. */
export interface DraftShown {
  from: Draft["from"];
  attachments: Draft["attachments"];
  /** The signature the draft carries, as marks; empty when it carries none. */
  signature: string;
}

function shownOf(draft: Draft): DraftShown {
  return { from: draft.from, attachments: draft.attachments, signature: draft.signature ?? "" };
}

function changesOf(
  fields: DraftFields,
  version: number,
  contentToken: string | null | undefined,
): DraftChanges {
  return { version, content_token: contentToken ?? null, ...fields };
}

/**
 * A draft being edited: local fields, saved in the background
 * ({@link AUTOSAVE_MS} after the last change, one save at a time, the
 * latest version named), then sent, discarded or taken back.
 */
export function useDraftEditor(workspaceId: string, draft: Draft) {
  const queryClient = useQueryClient();
  const deleteWithUndo = useDeleteDraft(workspaceId);
  const [fields, setFields] = useState<DraftFields>(() => fieldsOf(draft));
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [error, setError] = useState<string | undefined>(draft.failure_reason ?? undefined);
  const [sending, setSending] = useState(false);
  const [uploading, setUploading] = useState<UploadingFile[]>([]);
  // What it shows but does not edit (From, the attachments' names), from the
  // same answer as the token below.
  const [shown, setShown] = useState<DraftShown>(() => shownOf(draft));
  const version = useRef(draft.version);
  // The token of the content on screen (campbooks-cloud#242): taken only
  // from an answer whose content is what the editor shows (the draft it
  // opened with, a save of what it sent, a re-read it then shows), never
  // from a list or a read the editor does not show.
  const shownToken = useRef(draft.content_token);
  const latest = useRef(fields);
  const saved = useRef(fields);
  const inFlight = useRef<Promise<void> | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Uploads run one after the other, each after any save in flight.
  const uploads = useRef<Promise<unknown>>(Promise.resolve());
  const uploadKey = useRef(0);

  const refresh = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: draftKeys.all(workspaceId) });
  }, [queryClient, workspaceId]);

  /** Put `next` (or nothing, once discarded) in every cached list of drafts at once. */
  const replace = useCallback(
    (next: Draft | null) => {
      queryClient.setQueriesData<Draft[]>({ queryKey: draftKeys.all(workspaceId) }, (list) =>
        list?.flatMap((d) => (d.id === draft.id ? (next === null ? [] : [next]) : [d])),
      );
    },
    [draft.id, queryClient, workspaceId],
  );

  /**
   * Save what is there now, after any save already running. Resolves with
   * whether what is there now is saved.
   */
  const flush = useCallback(async (): Promise<boolean> => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    if (inFlight.current !== null) {
      await inFlight.current;
    }
    const current = latest.current;
    if (current === saved.current) {
      return true;
    }
    const run = async () => {
      setSaveState("saving");
      try {
        // A save keeps what it does not send (the attachments' files, the
        // reply headers), so it names the token of what is on screen.
        const answer = await saveDraft(
          workspaceId,
          draft.id,
          changesOf(current, version.current, shownToken.current),
        );
        version.current = answer.version;
        // What it saved is what is on screen: the answer's token is for it.
        shownToken.current = answer.content_token;
        setShown(shownOf(answer));
        saved.current = current;
        setSaveState(latest.current === current ? "saved" : "unsaved");
        setError(undefined);
      } catch (thrown) {
        if (!changedElsewhere(thrown)) {
          setSaveState("failed");
          return;
        }
        // Changed since it was shown (elsewhere, or opened before the server
        // checked): nothing was saved. Keep what the member typed, read the
        // draft again for its version, token and what it shows, and say so
        // once. The next save (an edit, or Send) saves the typed text over
        // it; nothing is saved again on its own.
        try {
          const fresh = await fetchDraft(workspaceId, draft.id);
          version.current = fresh.version;
          shownToken.current = fresh.content_token;
          setShown(shownOf(fresh));
        } catch {
          // The next save asks again.
        }
        setSaveState("unsaved");
        setError("content_changed");
      }
    };
    inFlight.current = run();
    await inFlight.current;
    inFlight.current = null;
    return saved.current === current;
  }, [draft.id, workspaceId]);

  const update = useCallback(
    (change: Partial<DraftFields>) => {
      setFields((previous) => {
        const next = { ...previous, ...change };
        latest.current = next;
        return next;
      });
      setSaveState("unsaved");
      if (timer.current !== null) {
        clearTimeout(timer.current);
      }
      timer.current = setTimeout(() => {
        void flush();
      }, AUTOSAVE_MS);
    },
    [flush],
  );

  // Leaving saves what is not saved yet.
  useEffect(
    () => () => {
      void flush();
    },
    [flush],
  );

  /**
   * Show `current` (the draft as the server holds it now) in place of what
   * is on screen, with its version and token: nothing local is lost, since
   * a send saves first.
   */
  const showCurrent = useCallback(
    (current: Draft) => {
      const next = fieldsOf(current);
      latest.current = next;
      saved.current = next;
      version.current = current.version;
      shownToken.current = current.content_token;
      setShown(shownOf(current));
      setFields(next);
      setSaveState("saved");
      replace(current);
    },
    [replace],
  );

  /**
   * Attach files from the device, in turn with the saves: what was typed is
   * saved first, then each file is uploaded naming the version and token the
   * last answer gave. An upload is a change to the draft, so its answer
   * becomes the draft shown (its version, token and attachments), and the
   * attachments the editor keeps include the new file, so the next save does
   * not drop it. A 409 is handled as a save's is: the draft is read again,
   * nothing typed is lost, and the notice says to check it. Resolves with the
   * files the server refused (and why), in order.
   */
  const attach = useCallback(
    (files: File[]): Promise<RefusedFile[]> => {
      const pending = files.map((file) => ({
        file,
        entry: { key: (uploadKey.current += 1), name: file.name, size: file.size },
      }));
      setUploading((list) => [...list, ...pending.map((p) => p.entry)]);
      const done = (key: number) => {
        setUploading((list) => list.filter((u) => u.key !== key));
      };
      const run = async (): Promise<RefusedFile[]> => {
        const refused: RefusedFile[] = [];
        for (const { file, entry } of pending) {
          try {
            // What was typed is saved first: the answer carries the draft as
            // the server holds it.
            await flush();
            const upload = async () => {
              try {
                const answer = await uploadAttachment(
                  workspaceId,
                  { id: draft.id, version: version.current, content_token: shownToken.current },
                  file,
                );
                version.current = answer.version;
                shownToken.current = answer.content_token;
                setShown(shownOf(answer));
                const attachments = answer.attachments.map((a) => a.id);
                // Only the attachments change: what was typed since stays
                // unsaved, and a draft with nothing unsaved stays saved.
                const clean = latest.current === saved.current;
                const nextSaved = { ...saved.current, attachments };
                saved.current = nextSaved;
                latest.current = clean ? nextSaved : { ...latest.current, attachments };
                setFields(latest.current);
              } catch (thrown) {
                if (!changedElsewhere(thrown)) {
                  refused.push({ file, code: problemCode(thrown) ?? "unknown" });
                  return;
                }
                // Not the draft that was shown: nothing was attached. Read it
                // again for its version and token, keep what was typed.
                refused.push({ file, code: problemCode(thrown) ?? "content_changed" });
                try {
                  const fresh = await fetchDraft(workspaceId, draft.id);
                  version.current = fresh.version;
                  shownToken.current = fresh.content_token;
                  setShown(shownOf(fresh));
                } catch {
                  // The next save or upload asks again.
                }
                setError("content_changed");
              }
            };
            inFlight.current = upload();
            await inFlight.current;
            inFlight.current = null;
          } finally {
            done(entry.key);
          }
        }
        return refused;
      };
      const next = uploads.current.then(run, run);
      uploads.current = next;
      return next;
    },
    [draft.id, flush, workspaceId],
  );

  /**
   * Leave the signature off (`false`) or add the member's current one back
   * (`true`): a save of its own, after any in flight and after what was typed
   * is saved, naming the version and token on screen. A draft changed
   * elsewhere is handled as a save's is.
   */
  const setSignature = useCallback(
    async (on: boolean): Promise<void> => {
      await flush();
      const run = async () => {
        setSaveState("saving");
        try {
          const answer = await saveDraft(workspaceId, draft.id, {
            version: version.current,
            content_token: shownToken.current ?? null,
            signature: on,
          });
          version.current = answer.version;
          shownToken.current = answer.content_token;
          setShown(shownOf(answer));
          setSaveState(latest.current === saved.current ? "saved" : "unsaved");
        } catch (thrown) {
          if (!changedElsewhere(thrown)) {
            setSaveState("failed");
            return;
          }
          try {
            const fresh = await fetchDraft(workspaceId, draft.id);
            version.current = fresh.version;
            shownToken.current = fresh.content_token;
            setShown(shownOf(fresh));
          } catch {
            // The next save asks again.
          }
          setSaveState("unsaved");
          setError("content_changed");
        }
      };
      inFlight.current = run();
      await inFlight.current;
      inFlight.current = null;
    },
    [draft.id, flush, workspaceId],
  );

  /**
   * Send: save first, then schedule, to leave when the undo window ends or at
   * `sendAt`. Resolves with the scheduled draft, or undefined.
   */
  const send = useCallback(
    async (sendAt?: Date): Promise<Draft | undefined> => {
      setSending(true);
      // Send only what the member sees: the server sends the saved version.
      const current = latest.current;
      try {
        if (!(await flush()) || latest.current !== current) {
          // A save refused as changed has said so already; keep that notice.
          setError((shownError) => (shownError === "content_changed" ? shownError : "not_saved"));
          return undefined;
        }
        const scheduled = await sendDraft(
          workspaceId,
          draft.id,
          { version: version.current, contentToken: shownToken.current },
          sendAt,
        );
        replace(scheduled);
        void refresh();
        return scheduled;
      } catch (thrown) {
        if (!changedElsewhere(thrown)) {
          setError(problemCode(thrown) ?? "unknown");
          return undefined;
        }
        // Not the draft the member was shown: saved since elsewhere (another
        // tab, another device, the assistant: `draft_changed`) or changed
        // under it (`content_changed`). Nothing was sent. Read it again and
        // show it as it is now, so the next Send names its version and token
        // (campbooks-cloud#242); never sent on their behalf.
        try {
          const fresh = await fetchDraft(workspaceId, draft.id);
          if (latest.current === current) {
            showCurrent(fresh);
          } else {
            // Typed while the send was refused: keep it, take only the
            // version and token, and the next save writes it over them.
            version.current = fresh.version;
            shownToken.current = fresh.content_token;
            setShown(shownOf(fresh));
          }
        } catch {
          // The notice still says to check it; the next send asks again.
        }
        setError("content_changed");
        return undefined;
      } finally {
        setSending(false);
      }
    },
    [draft.id, flush, refresh, replace, showCurrent, workspaceId],
  );

  /** Take a scheduled send back. Whether it was in time. */
  const undo = useCallback(async (): Promise<boolean> => {
    try {
      const back = await cancelSend(workspaceId, draft.id);
      version.current = back.version;
      // Its token is not taken: the draft shown again opens with its own.
      replace(back);
      return true;
    } catch {
      return false;
    } finally {
      void refresh();
    }
  }, [draft.id, refresh, replace, workspaceId]);

  /**
   * Throw the draft away, with Undo in a toast. Undo writes it back as it
   * was on screen, unsaved edits included: a save in flight finishes first,
   * a pending one is dropped.
   */
  const discard = useCallback(async () => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    if (inFlight.current !== null) {
      await inFlight.current;
    }
    const seen = latest.current;
    // Nothing may save into a draft that is going.
    latest.current = saved.current;
    try {
      await deleteWithUndo({
        ...draft,
        ...seen,
        // Uploaded files too: Undo brings the draft back without them (their
        // blobs are released), and says so.
        attachments: shown.attachments.filter((a) => seen.attachments.includes(a.id)),
      });
    } catch (error) {
      // Refused: the draft stays, and what was typed is still to be saved
      // (the next save or Send writes it, never the last saved version).
      latest.current = seen;
      throw error;
    }
  }, [deleteWithUndo, draft, shown.attachments]);

  return {
    fields,
    shown,
    update,
    attach,
    uploading,
    setSignature,
    saveState,
    error,
    sending,
    send,
    undo,
    discard,
    flush,
  };
}
