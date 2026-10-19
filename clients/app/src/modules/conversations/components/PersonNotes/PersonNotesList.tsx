import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  EmptyState,
  List,
  ListRow,
  SettingsEmpty,
  SkeletonList,
  Text,
  useUndoToast,
  VisuallyHidden,
} from "~/lib/ui";

import { usePersonNoteMutations, usePersonNotes, type PersonNote } from "../../api/person-notes";
import type { Translate } from "../../model";

import { NoteForm, type NoteFormEnd } from "./NoteForm";

/**
 * What the member told Scout about people (campbooks-cloud#272), the newest
 * change first: each person by name (or address) with the note, Edit in
 * place (the same field as the reply dialog's) and Remove, with Undo. Theirs
 * alone. The people slot of Scout's memory.
 * @public
 */
export function PersonNotesList({ workspaceId }: { workspaceId: string }) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const notes = usePersonNotes(workspaceId);
  const [said, setSaid] = useState("");

  if (notes.isError) {
    return (
      <EmptyState
        title={t("personNotes.list.failed")}
        visor="thinking"
        headingLevel={3}
        action={
          <Button
            variant="secondary"
            onClick={() => {
              void notes.refetch();
            }}
          >
            {t("personNotes.list.retry")}
          </Button>
        }
      />
    );
  }
  if (notes.data === undefined) {
    return <SkeletonList rows={2} />;
  }
  return (
    <>
      {notes.data.length === 0 ? (
        <SettingsEmpty title={t("personNotes.list.emptyTitle")}>
          {t("personNotes.list.empty")}
        </SettingsEmpty>
      ) : (
        <List label={t("personNotes.list.label")}>
          {notes.data.map((note) => (
            <NoteRow
              key={note.id}
              workspaceId={workspaceId}
              note={note}
              onSaved={() => {
                setSaid(t("personNotes.list.saved"));
              }}
            />
          ))}
        </List>
      )}
      <div role="status">
        <VisuallyHidden>{said}</VisuallyHidden>
      </div>
    </>
  );
}

function NoteRow({
  workspaceId,
  note,
  onSaved,
}: {
  workspaceId: string;
  note: PersonNote;
  onSaved: () => void;
}) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const undo = useUndoToast();
  const { keep, forget } = usePersonNoteMutations(workspaceId);
  const [editing, setEditing] = useState(false);
  const edit = useRef<HTMLButtonElement>(null);
  const back = useRef(false);
  const name = note.name ?? note.address;

  useEffect(() => {
    if (!editing && back.current) {
      back.current = false;
      edit.current?.focus();
    }
  }, [editing]);

  const failed = () => {
    undo.notify(t("personNotes.toast.failed"));
  };

  // Awaited rather than per-call callbacks: the row is gone once its note
  // is, before they would run.
  const remove = async () => {
    const gone = note;
    try {
      await forget.mutateAsync({ noteId: gone.id });
    } catch {
      failed();
      return;
    }
    undo.push({
      label: t("personNotes.toast.forgotten", { name }),
      undo: () => {
        keep.mutateAsync({ address: gone.address, note: gone.note }).catch(failed);
      },
    });
  };

  const end = (how: NoteFormEnd) => {
    back.current = how !== "forgotten";
    setEditing(false);
    if (how === "saved") {
      onSaved();
    }
  };

  return (
    <ListRow
      title={name}
      meta={note.name === undefined || note.name === null ? undefined : note.address}
      trailing={
        editing ? undefined : (
          <>
            <Button
              ref={edit}
              variant="ghost"
              aria-label={t("personNotes.list.editLabel", { name })}
              onClick={() => {
                setEditing(true);
              }}
            >
              {t("personNotes.list.edit")}
            </Button>
            <Button
              variant="ghost"
              aria-label={t("personNotes.list.removeLabel", { name })}
              disabled={forget.isPending}
              onClick={() => {
                void remove();
              }}
            >
              {t("personNotes.list.remove")}
            </Button>
          </>
        )
      }
    >
      {editing ? (
        <NoteForm
          workspaceId={workspaceId}
          address={note.address}
          name={name}
          note={note}
          onEnd={end}
        />
      ) : (
        <Text size="sm" tone="secondary">
          {note.note}
        </Text>
      )}
    </ListRow>
  );
}
