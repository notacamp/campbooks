import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, TextField } from "~/lib/ui";

import { readSender } from "../../model";

export interface AddSenderProps {
  /** Called with a lower-cased address or domain the member typed. */
  onAdd: (sender: string) => void;
  /** Field label (defaults to "Someone else"). */
  label?: string | undefined;
}

/**
 * "Someone else": an address or a domain, typed. Enter adds it (inside a
 * page whose form Enter submits, so it doesn't move on). A free mail domain
 * is refused, with why.
 */
export function AddSender({ onAdd, label }: AddSenderProps) {
  const { t } = useTranslation("assistant");
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | undefined>();
  const add = () => {
    const typed = readSender(value);
    switch (typed.kind) {
      case "address":
      case "domain": {
        onAdd(typed.value);
        setValue("");
        setError(undefined);
        return;
      }
      case "free_mail": {
        setError(t("flow.add.freeMail", { domain: typed.value }));
        return;
      }
      case "invalid": {
        setError(t("flow.add.invalid"));
      }
    }
  };
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
      <div className="min-w-0 flex-1">
        <TextField
          label={label ?? t("flow.add.label")}
          placeholder={t("flow.add.placeholder")}
          autoComplete="off"
          inputMode="email"
          value={value}
          error={error}
          onChange={(event) => {
            setValue(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              if (value.trim().length > 0) {
                add();
              }
            }
          }}
        />
      </div>
      <Button disabled={value.trim().length === 0} onClick={add}>
        {t("flow.add.button")}
      </Button>
    </div>
  );
}
