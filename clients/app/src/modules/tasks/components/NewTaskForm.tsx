import { useEffect, useState, type RefObject, type SyntheticEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  Heading,
  PaneHeader,
  ReaderBody,
  Segmented,
  Spacer,
  Text,
  TextField,
} from "~/lib/ui";

import type { NewTask, TaskVerb } from "../api/queries";

import { DueChips } from "./DueChips";

export interface NewTaskFormProps {
  /** The title field, so `n t` can bring the focus back to it. */
  titleRef: RefObject<HTMLInputElement | null>;
  /** Make the task; resolves when the server answered. */
  onAdd: (body: NewTask) => Promise<unknown>;
  /** Back to the list (Escape, Cancel, Close, Back). */
  onClose: () => void;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * A new task, in the right pane, in the shape of a task's page: the title as
 * the page's title, Do or Pay, when it is due, then Add task. Adding keeps
 * the form open and empty, the title focused for the next one. A date is
 * never read from the words typed: a chip or the month grid gives it.
 */
export function NewTaskForm({ titleRef, onAdd, onClose, now }: NewTaskFormProps) {
  const { t: typed } = useTranslation("tasks");
  const t = typed as unknown as (key: string) => string;
  const [title, setTitle] = useState("");
  const [due, setDue] = useState<string | undefined>();
  const [verb, setVerb] = useState<TaskVerb>("do");
  useEffect(() => {
    titleRef.current?.focus();
  }, [titleRef]);

  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    if (title.trim() === "") {
      return;
    }
    const body: NewTask = {
      title,
      ...(verb === "pay" ? { verb } : {}),
      ...(due === undefined ? {} : { due_on: due }),
    };
    setTitle("");
    setDue(undefined);
    setVerb("do");
    void onAdd(body);
    titleRef.current?.focus();
  };

  return (
    <>
      <PaneHeader>
        <span className="flex lg:hidden">
          <Button variant="ghost" icon="chevron-left" shortcut="overlay.close" onClick={onClose}>
            {t("detail.back")}
          </Button>
        </span>
        <Spacer />
        <span className="hidden lg:flex">
          <Button variant="ghost" onClick={onClose}>
            {t("form.close")}
          </Button>
        </span>
      </PaneHeader>
      <ReaderBody>
        <form className="flex flex-col gap-5" onSubmit={submit} aria-label={t("form.label")}>
          <TextField
            ref={titleRef}
            label={t("detail.titleLabel")}
            hideLabel
            size="title"
            placeholder={t("form.placeholder")}
            autoComplete="off"
            value={title}
            onChange={(event) => {
              setTitle(event.target.value);
            }}
          />
          <div className="flex items-center gap-3">
            <Text as="span" tone="secondary" size="sm">
              {t("add.kind")}
            </Text>
            <Segmented
              label={t("add.kind")}
              value={verb}
              onChange={setVerb}
              options={[
                { value: "do", label: t("add.do") },
                { value: "pay", label: t("add.pay") },
              ]}
            />
          </div>
          <section className="flex flex-col gap-2">
            <Heading level={3} size="section">
              {t("form.due")}
            </Heading>
            <DueChips label={t("add.due")} value={due} onChange={setDue} now={now} />
          </section>
          <div className="flex items-center gap-2">
            <Button type="submit" variant="primary" keys="enter" disabled={title.trim() === ""}>
              {t("make.submit")}
            </Button>
            <Button variant="secondary" shortcut="overlay.close" onClick={onClose}>
              {t("make.cancel")}
            </Button>
          </div>
        </form>
      </ReaderBody>
    </>
  );
}
