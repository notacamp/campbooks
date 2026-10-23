import { useState, type RefObject, type SyntheticEvent } from "react";
import { useTranslation } from "react-i18next";

import { Button, Segmented, TextField } from "~/lib/ui";

import type { NewTask, TaskVerb } from "../api/queries";

import { DueChips } from "./DueChips";

export interface QuickAddProps {
  /** The field, so `n t` can focus it. */
  inputRef: RefObject<HTMLInputElement | null>;
  /** Make the task; resolves when the server answered. */
  onAdd: (body: NewTask) => Promise<unknown>;
  now?: Date | undefined;
}

/**
 * "Add a task…": a title, a due chip and Do or Pay. Enter adds it, empties
 * the field and keeps it focused for the next one.
 */
export function QuickAdd({ inputRef, onAdd, now }: QuickAddProps) {
  const { t: typed } = useTranslation("tasks");
  const t = typed as unknown as (key: string) => string;
  const [title, setTitle] = useState("");
  const [due, setDue] = useState<string | undefined>();
  const [verb, setVerb] = useState<TaskVerb>("do");
  const [touched, setTouched] = useState(false);
  // The chips show once the field has been used: there is something to give a date to.
  const open = touched || title !== "" || due !== undefined;

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
    inputRef.current?.focus();
  };

  return (
    <form className="flex flex-col gap-2 px-3 pb-2" onSubmit={submit}>
      <TextField
        ref={inputRef}
        label={t("add.label")}
        hideLabel
        placeholder={t("add.placeholder")}
        autoComplete="off"
        onFocus={() => {
          setTouched(true);
        }}
        value={title}
        onChange={(event) => {
          setTitle(event.target.value);
        }}
      />
      {open ? (
        <div className="flex flex-col gap-2">
          <DueChips
            label={t("add.due")}
            value={due}
            onChange={setDue}
            now={now}
            idPrefix="quick-add"
          />
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
            <Button type="submit" variant="primary" keys="enter" disabled={title.trim() === ""}>
              {t("add.submit")}
            </Button>
          </div>
        </div>
      ) : null}
    </form>
  );
}
