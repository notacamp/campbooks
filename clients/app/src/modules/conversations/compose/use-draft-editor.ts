import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";

import { problemCode } from "~/lib/api";

import {
  cancelSend,
  deleteDraft,
  draftKeys,
  fetchDraft,
  saveDraft,
  sendDraft,
  type Draft,
  type DraftChanges,
} from "../api/drafts";

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

function changesOf(fields: DraftFields, version: number): DraftChanges {
  return { version, ...fields };
}

/**
 * A draft being edited: local fields, saved in the background
 * ({@link AUTOSAVE_MS} after the last change, one save at a time, the
 * latest version named), then sent, discarded or taken back.
 */
export function useDraftEditor(workspaceId: string, draft: Draft) {
  const queryClient = useQueryClient();
  const [fields, setFields] = useState<DraftFields>(() => fieldsOf(draft));
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [error, setError] = useState<string | undefined>(draft.failure_reason ?? undefined);
  const [sending, setSending] = useState(false);
  const version = useRef(draft.version);
  const latest = useRef(fields);
  const saved = useRef(fields);
  const inFlight = useRef<Promise<void> | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  /** Save what is there now, after any save already running. */
  const flush = useCallback(async (): Promise<void> => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    if (inFlight.current !== null) {
      await inFlight.current;
    }
    const current = latest.current;
    if (current === saved.current) {
      return;
    }
    const run = async () => {
      setSaveState("saving");
      try {
        let answer: Draft;
        try {
          answer = await saveDraft(workspaceId, draft.id, changesOf(current, version.current));
        } catch (thrown) {
          if (problemCode(thrown) !== "draft_changed") {
            throw thrown;
          }
          // Saved elsewhere meanwhile (another tab): this edit is the latest.
          version.current = (await fetchDraft(workspaceId, draft.id)).version;
          answer = await saveDraft(workspaceId, draft.id, changesOf(current, version.current));
        }
        version.current = answer.version;
        saved.current = current;
        setSaveState(latest.current === current ? "saved" : "unsaved");
        setError(undefined);
      } catch {
        setSaveState("failed");
      }
    };
    inFlight.current = run();
    await inFlight.current;
    inFlight.current = null;
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

  /** Send: save first, then schedule. Resolves with the scheduled draft, or undefined. */
  const send = useCallback(async (): Promise<Draft | undefined> => {
    setSending(true);
    try {
      await flush();
      const scheduled = await sendDraft(workspaceId, draft.id);
      replace(scheduled);
      void refresh();
      return scheduled;
    } catch (thrown) {
      setError(problemCode(thrown) ?? "unknown");
      return undefined;
    } finally {
      setSending(false);
    }
  }, [draft.id, flush, refresh, replace, workspaceId]);

  /** Take a scheduled send back. Whether it was in time. */
  const undo = useCallback(async (): Promise<boolean> => {
    try {
      const back = await cancelSend(workspaceId, draft.id);
      version.current = back.version;
      replace(back);
      return true;
    } catch {
      return false;
    } finally {
      void refresh();
    }
  }, [draft.id, refresh, replace, workspaceId]);

  /** Throw the draft away. */
  const discard = useCallback(async () => {
    if (timer.current !== null) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    latest.current = saved.current;
    await deleteDraft(workspaceId, draft.id);
    replace(null);
    await refresh();
  }, [draft.id, refresh, replace, workspaceId]);

  return { fields, update, saveState, error, sending, send, undo, discard, flush };
}
