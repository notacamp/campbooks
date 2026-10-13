import { useId, useState, type KeyboardEvent, type ReactNode, type Ref } from "react";
import { useTranslation } from "react-i18next";

import { Icon } from "../Icon/Icon";

import styles from "./RecipientField.module.css";

/** Someone a message goes to. */
export interface Recipient {
  name?: string | null | undefined;
  address: string;
}

/** Whether `address` looks like one mail can be sent to (the server checks again). */
export function isEmailAddress(address: string): boolean {
  return /^[^\s@<>,;"()[\]\\]+@[^\s@<>,;"()[\]\\.]+(?:\.[^\s@<>,;"()[\]\\.]+)+$/u.test(address);
}

/** "Ana Lopes <ana@example.com>", "ana@example.com" or a list of either, as recipients. */
export function parseRecipients(text: string): Recipient[] {
  return text
    .split(/[,;\n]/u)
    .map((part) => part.trim())
    .filter((part) => part !== "")
    .map((part) => {
      const match = /^(?:"?([^"<]*?)"?\s*)?<([^>]+)>$/u.exec(part);
      if (match === null) {
        return { address: part };
      }
      const name = (match[1] ?? "").trim();
      return { name: name === "" ? null : name, address: (match[2] ?? "").trim() };
    });
}

export interface RecipientFieldProps {
  /** "To", "Cc", "Bcc": the visible label and the accessible name of the input. */
  label: string;
  recipients: readonly Recipient[];
  onChange: (recipients: Recipient[]) => void;
  /** Validity of a typed address; defaults to {@link isEmailAddress}. */
  isValid?: ((address: string) => boolean) | undefined;
  placeholder?: string | undefined;
  /** The text input, to focus it. */
  inputRef?: Ref<HTMLInputElement> | undefined;
  /** Drawn at the end of the row (the "Cc Bcc" toggle). */
  trailing?: ReactNode | undefined;
}

/**
 * Addresses as removable chips. Typing an address and pressing Enter, Tab,
 * a comma or leaving the field adds it; pasting a list adds each one;
 * Backspace in the empty field removes the last chip.
 */
export function RecipientField({
  label,
  recipients,
  onChange,
  isValid = isEmailAddress,
  placeholder,
  inputRef,
  trailing,
}: RecipientFieldProps) {
  const { t } = useTranslation("ui");
  const id = useId();
  const errorId = `${id}-error`;
  const [text, setText] = useState("");
  const [invalid, setInvalid] = useState<string | null>(null);

  /** Turn `value` into chips; returns whether everything was valid. */
  function commit(value: string): boolean {
    const parsed = parseRecipients(value);
    if (parsed.length === 0) {
      setText("");
      return true;
    }
    const bad = parsed.find((recipient) => !isValid(recipient.address));
    if (bad !== undefined) {
      setInvalid(bad.address);
      return false;
    }
    const known = new Set(recipients.map((r) => r.address.toLowerCase()));
    const fresh = parsed.filter((r) => {
      const key = r.address.toLowerCase();
      if (known.has(key)) {
        return false;
      }
      known.add(key);
      return true;
    });
    if (fresh.length > 0) {
      onChange([...recipients, ...fresh]);
    }
    setText("");
    setInvalid(null);
    return true;
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" || event.key === "," || event.key === ";") {
      if (text.trim() !== "") {
        event.preventDefault();
        commit(text);
      } else if (event.key !== "Enter") {
        event.preventDefault();
      }
    } else if (event.key === "Tab" && text.trim() !== "") {
      if (!commit(text)) {
        event.preventDefault();
      }
    } else if (event.key === "Backspace" && text === "" && recipients.length > 0) {
      onChange(recipients.slice(0, -1));
    }
  }

  return (
    <div className={styles["field"]}>
      <div className={styles["row"]} data-invalid={invalid === null ? undefined : true}>
        <label className={styles["label"]} htmlFor={id}>
          {label}
        </label>
        <ul className={styles["chips"]} aria-label={t("recipients.list", { label })}>
          {recipients.map((recipient) => (
            <li key={recipient.address.toLowerCase()} className={styles["chip"]}>
              <span className={styles["chipText"]} title={recipient.address}>
                {recipient.name ?? recipient.address}
              </span>
              <button
                type="button"
                className={styles["remove"]}
                aria-label={t("recipients.remove", {
                  name: recipient.name ?? recipient.address,
                })}
                onClick={() => {
                  onChange(recipients.filter((r) => r !== recipient));
                }}
              >
                <Icon name="close" />
              </button>
            </li>
          ))}
          <li className={styles["entry"]}>
            <input
              id={id}
              ref={inputRef}
              className={styles["input"]}
              type="text"
              inputMode="email"
              autoComplete="email"
              spellCheck={false}
              placeholder={recipients.length === 0 ? placeholder : undefined}
              value={text}
              aria-invalid={invalid === null ? undefined : true}
              aria-describedby={invalid === null ? undefined : errorId}
              onChange={(event) => {
                const value = event.target.value;
                if (/[,;\n]/u.test(value) && value.trim().length > 1) {
                  commit(value);
                } else {
                  setText(value);
                  setInvalid(null);
                }
              }}
              onKeyDown={onKeyDown}
              onBlur={() => {
                if (text.trim() !== "") {
                  commit(text);
                }
              }}
            />
          </li>
        </ul>
        {trailing}
      </div>
      {invalid === null ? null : (
        <p id={errorId} className={styles["error"]}>
          <Icon name="warning" />
          <span>{t("recipients.invalid", { address: invalid })}</span>
        </p>
      )}
    </div>
  );
}
