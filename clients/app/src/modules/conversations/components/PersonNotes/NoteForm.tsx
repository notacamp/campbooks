import { useState } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { Button, NoteField, useUndoToast } from "~/lib/ui";

import { MAX_NOTE_CHARS, usePersonNoteMutations, type PersonNote } from "../../api/person-notes";
import type { Translate } from "../../model";

/** How the form closed. */
export type NoteFormEnd = "saved" | "forgotten" | "cancelled";

export interface NoteFormProps {
  workspaceId: string;
  /** The person's address (lower-cased): what the note is kept by. */
  address: string;
  /** How the person is called here ("Marta"). */
  name: string;
  /** The note kept now, when there is one. */
  note?: PersonNote | undefined;
  /** Saved, forgotten or left: the caller puts its line back. */
  onEnd: (end: NoteFormEnd) => void;
}

/**
 * The one field to tell Scout about a person, inline (never a dialog of its
 * own): Save (⌘↵), Cancel (Esc) and, for a note kept, Forget this. Saving and
 * forgetting say so with Undo, which puts the note back as it was.
 */
export function NoteForm({ workspaceId, address, name, note, onEnd }: NoteFormProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undo = useUndoToast();
  const { keep, forget } = usePersonNoteMutations(workspaceId);
  const [value, setValue] = useState(note?.note ?? "");
  const [error, setError] = useState<string | undefined>();
  const busy = keep.isPending || forget.isPending;
  const failed = () => {
    undo.notify(t("personNotes.toast.failed"));
  };

  /** Put back what was there before: the old words, or no note at all. */
  const putBack = (before: string | undefined, keptId: string) => {
    const done =
      before === undefined
        ? forget.mutateAsync({ noteId: keptId })
        : keep.mutateAsync({ address, note: before });
    done.catch(failed);
  };

  // Awaited rather than per-call callbacks: the row holding this form may
  // be gone (its note forgotten) before they would run.
  const save = async () => {
    const words = value.trim();
    if (busy || words === "") {
      return;
    }
    setError(undefined);
    const before = note?.note;
    const kept = await keep.mutateAsync({ address, note: words }).catch((thrown: unknown) => {
      const code = problemCode(thrown);
      setError(
        t(
          code === "person_notes_full"
            ? "personNotes.form.full"
            : code === "person_note_invalid"
              ? "personNotes.form.invalid"
              : "personNotes.form.failed",
        ),
      );
      return null;
    });
    if (kept === null) {
      return;
    }
    onEnd("saved");
    undo.push({
      label: t("personNotes.toast.saved", { name }),
      undo: () => {
        putBack(before, kept.id);
      },
    });
  };

  const forgetIt = async () => {
    if (busy || note === undefined) {
      return;
    }
    const gone = note;
    try {
      await forget.mutateAsync({ noteId: gone.id });
    } catch {
      setError(t("personNotes.form.failed"));
      return;
    }
    onEnd("forgotten");
    undo.push({
      label: t("personNotes.toast.forgotten", { name }),
      undo: () => {
        keep.mutateAsync({ address: gone.address, note: gone.note }).catch(failed);
      },
    });
  };

  const cancel = () => {
    if (!busy) {
      onEnd("cancelled");
    }
  };

  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <NoteField
        label={t("personNotes.form.label", { name })}
        placeholder={t("personNotes.form.placeholder")}
        value={value}
        onChange={setValue}
        maxLength={MAX_NOTE_CHARS}
        counter={(left) => t("personNotes.form.left", { count: left, n: left })}
        onSubmit={() => {
          void save();
        }}
        onCancel={cancel}
        disabled={busy}
        error={error}
      />
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="submit"
          variant="primary"
          keys="mod+enter"
          disabled={busy || value.trim() === ""}
        >
          {t("personNotes.form.save")}
        </Button>
        <Button variant="ghost" shortcut="overlay.close" disabled={busy} onClick={cancel}>
          {t("personNotes.form.cancel")}
        </Button>
        {note === undefined ? null : (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => {
              void forgetIt();
            }}
          >
            {t("personNotes.form.forget")}
          </Button>
        )}
      </div>
    </form>
  );
}
