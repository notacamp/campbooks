import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type SyntheticEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button, Segmented, TextField } from "~/lib/ui";

import type { NewTask, TaskVerb } from "../../api/queries";
import { CONVERSATION } from "../../source";
import { DueChips } from "../DueChips";
import { useTaskActions } from "../use-task-actions";

export interface MakeTaskFormProps {
  workspaceId: string;
  conversationId: string;
  /** The subject, which the title starts as. */
  subject: string;
  /** The sender's address, kept to recognise their other tasks; never shown. */
  senderAddress?: string | undefined;
  /** The form is done with (cancelled, or closed after the task was added). */
  onClose: () => void;
  now?: Date | undefined;
}

/**
 * "Make it a task", inline in the reader (not a dialog): the subject as the
 * title, the due chips and Do or Pay. Add makes the task from the
 * conversation, says so with Undo and Open, and Escape closes the form.
 */
export function MakeTaskForm({
  workspaceId,
  conversationId,
  subject,
  senderAddress,
  onClose,
  now,
}: MakeTaskFormProps) {
  const { t: typed } = useTranslation("tasks");
  const t = typed as unknown as (key: string) => string;
  const navigate = useNavigate();
  const actions = useTaskActions(workspaceId, { now });
  const [title, setTitle] = useState(subject);
  const [due, setDue] = useState<string | undefined>();
  const [verb, setVerb] = useState<TaskVerb>("do");
  const [adding, setAdding] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, []);
  useShortcut("overlay.close", onClose);

  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    if (title.trim() === "" || adding) {
      return;
    }
    const body: NewTask = {
      title,
      ...(verb === "pay" ? { verb } : {}),
      ...(due === undefined ? {} : { due_on: due }),
      source: { type: CONVERSATION, id: conversationId },
      ...(senderAddress === undefined ? {} : { source_sender: senderAddress }),
    };
    setAdding(true);
    // The form closes once the task is made; the toast says so, with Undo
    // and Open. A failure keeps the form, with the toast's Retry.
    void actions
      .add(body, t("make.added"), {
        label: t("make.open"),
        run: (made) => {
          void navigate({ to: `/w/${workspaceId}/tasks/${made.id}` });
        },
      })
      .then((made) => {
        setAdding(false);
        if (made !== undefined) {
          onClose();
        }
      });
  };

  return (
    <form className="flex flex-col gap-3" onSubmit={submit} aria-label={t("make.button")}>
      <TextField
        ref={field}
        label={t("make.title")}
        autoComplete="off"
        value={title}
        onChange={(event) => {
          setTitle(event.target.value);
        }}
      />
      <DueChips label={t("add.due")} value={due} onChange={setDue} now={now} />
      <div className="flex flex-wrap items-center gap-2">
        <Segmented
          label={t("add.kind")}
          value={verb}
          onChange={setVerb}
          options={[
            { value: "do", label: t("add.do") },
            { value: "pay", label: t("add.pay") },
          ]}
        />
        <Button
          type="submit"
          variant="secondary"
          keys="enter"
          disabled={title.trim() === "" || adding}
        >
          {t("make.submit")}
        </Button>
        <Button variant="secondary" shortcut="overlay.close" onClick={onClose}>
          {t("make.cancel")}
        </Button>
      </div>
    </form>
  );
}
