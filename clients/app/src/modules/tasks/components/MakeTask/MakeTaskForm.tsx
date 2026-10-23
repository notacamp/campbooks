import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type SyntheticEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button, Segmented, Text, TextField } from "~/lib/ui";

import type { NewTask, TaskVerb, TaskView } from "../../api/queries";
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
  const [added, setAdded] = useState<TaskView | undefined>();
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
    void actions.add(body, t("make.added")).then((made) => {
      setAdding(false);
      setAdded(made);
    });
  };

  if (added !== undefined) {
    return (
      <div className="flex flex-wrap items-center gap-2" role="status">
        <Text>{t("make.added")}</Text>
        <Button
          variant="link"
          onClick={() => {
            void navigate({ to: `/w/${workspaceId}/tasks/${added.id}` });
          }}
        >
          {t("make.open")}
        </Button>
        <Button variant="link" onClick={onClose}>
          {t("make.close")}
        </Button>
      </div>
    );
  }
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
      <DueChips label={t("add.due")} value={due} onChange={setDue} now={now} idPrefix="make-task" />
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
          variant="primary"
          keys="enter"
          disabled={title.trim() === "" || adding}
        >
          {t("make.submit")}
        </Button>
        <Button variant="ghost" shortcut="overlay.close" onClick={onClose}>
          {t("make.cancel")}
        </Button>
      </div>
    </form>
  );
}
