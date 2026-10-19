import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Text, VisuallyHidden } from "~/lib/ui";

import { usePersonNotes } from "../../api/person-notes";
import type { Translate } from "../../model";

import { NoteForm, type NoteFormEnd } from "./NoteForm";

export interface AboutPersonProps {
  workspaceId: string;
  /** The person answered: their lower-cased plain address. */
  address: string;
  /** How they are called here ("Marta", or their address). */
  name: string;
  /** While a draft shows: write it again, so the note is seen at work. */
  onRewrite?: (() => void) | undefined;
}

/**
 * One quiet line in the reply dialog (campbooks-cloud#272): "Tell Scout
 * about Marta", or what Scout knows about her with Edit. It opens the note's
 * field in place; after Save or Cancel the focus is back on the line, and a
 * live region says it was saved. Once saved while a draft shows, "Rewrite
 * with this" writes it again with the note (only on that click). Nothing
 * shows while the notes load or if they can't: the note only steers Scout,
 * and the server reads it whatever the line shows.
 */
export function AboutPerson({ workspaceId, address, name, onRewrite }: AboutPersonProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const notes = usePersonNotes(workspaceId);
  const note = notes.data?.find((item) => item.address === address);
  const [editing, setEditing] = useState(false);
  const [said, setSaid] = useState("");
  const [offer, setOffer] = useState(false);
  const line = useRef<HTMLButtonElement>(null);
  const back = useRef(false);

  useEffect(() => {
    if (!editing && back.current) {
      back.current = false;
      line.current?.focus();
    }
  }, [editing]);

  if (notes.data === undefined) {
    return null;
  }

  const end = (how: NoteFormEnd) => {
    back.current = true;
    setEditing(false);
    setSaid(how === "saved" ? t("personNotes.line.saved", { name }) : "");
    setOffer(how === "saved");
  };

  return (
    <div className="flex min-w-0 flex-col gap-2">
      {editing ? (
        <NoteForm workspaceId={workspaceId} address={address} name={name} note={note} onEnd={end} />
      ) : (
        <div className="flex min-w-0 items-center gap-2">
          {note === undefined ? (
            <Button
              ref={line}
              variant="link"
              ai
              onClick={() => {
                setEditing(true);
              }}
            >
              {t("personNotes.line.tell", { name })}
            </Button>
          ) : (
            <>
              <Text as="span" size="sm" tone="secondary" truncate className="min-w-0 flex-1">
                {t("personNotes.line.knows", { note: note.note })}
              </Text>
              <Button
                ref={line}
                variant="link"
                aria-label={t("personNotes.line.editLabel", { name })}
                onClick={() => {
                  setEditing(true);
                }}
              >
                {t("personNotes.line.edit")}
              </Button>
            </>
          )}
        </div>
      )}
      {offer && !editing && onRewrite !== undefined ? (
        <div>
          <Button
            ai
            onClick={() => {
              setOffer(false);
              onRewrite();
            }}
          >
            {t("personNotes.line.rewrite")}
          </Button>
        </div>
      ) : null}
      <div role="status">
        <VisuallyHidden>{said}</VisuallyHidden>
      </div>
    </div>
  );
}
