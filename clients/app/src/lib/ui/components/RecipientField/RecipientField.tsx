import { useEffect, useId, useState, type KeyboardEvent, type ReactNode, type Ref } from "react";
import { useTranslation } from "react-i18next";

import { Avatar } from "../Avatar/Avatar";
import { Icon } from "../Icon/Icon";
import { KbdHint } from "../Kbd/Kbd";

import styles from "./RecipientField.module.css";

/** Someone a message goes to. */
export interface Recipient {
  name?: string | null;
  address: string;
}

/** Someone suggested as you type: a person you exchange mail with. */
export interface RecipientSuggestion {
  name?: string | null;
  address: string;
}

/** Most suggestions listed at once. */
const MAX_SUGGESTIONS = 8;

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
  /**
   * People to suggest for what is being typed (from `onQueryChange`). Given
   * this or `onQueryChange`, the field is a combobox: a list under it, ↑ ↓
   * to move, Enter or Tab to pick, Esc to close. Those already in
   * `recipients` are left out, and at most {@link MAX_SUGGESTIONS} shown.
   */
  suggestions?: readonly RecipientSuggestion[] | undefined;
  /** What is being typed, as it changes (`""` once it is added). */
  onQueryChange?: ((text: string) => void) | undefined;
  /** Suggestions for the text are on their way. */
  loadingSuggestions?: boolean | undefined;
}

/**
 * Addresses as removable chips. Typing an address and pressing Enter, Tab,
 * a comma or leaving the field adds it; pasting a list adds each one;
 * Backspace in the empty field removes the last chip. With suggestions, it
 * is an ARIA combobox: the people suggested for what is typed are listed
 * under it, focus stays in the input, ↑ ↓ move, Enter or Tab add the
 * highlighted person, a click or a tap adds one, and Esc closes the list
 * (and only the list: with it closed, Esc is left to whatever holds the
 * field).
 */
export function RecipientField({
  label,
  recipients,
  onChange,
  isValid = isEmailAddress,
  placeholder,
  inputRef,
  trailing,
  suggestions,
  onQueryChange,
  loadingSuggestions = false,
}: RecipientFieldProps) {
  const { t } = useTranslation("ui");
  const id = useId();
  const errorId = `${id}-error`;
  const listId = `${id}-suggestions`;
  const [text, setText] = useState("");
  const [invalid, setInvalid] = useState<string | null>(null);
  // Esc, leaving the field or a pick closes the list; typing opens it again.
  const [open, setOpen] = useState(true);
  const [active, setActive] = useState(0);

  const suggests = suggestions !== undefined || onQueryChange !== undefined;
  const chosen = new Set(recipients.map((r) => r.address.toLowerCase()));
  const listed: RecipientSuggestion[] = [];
  for (const suggestion of suggestions ?? []) {
    const key = suggestion.address.toLowerCase();
    if (!chosen.has(key) && listed.length < MAX_SUGGESTIONS) {
      chosen.add(key);
      listed.push(suggestion);
    }
  }
  const typing = suggests && open && text.trim() !== "";
  const expanded = typing && listed.length > 0;
  const searching = typing && listed.length === 0 && loadingSuggestions;
  const activeIndex = Math.min(active, Math.max(listed.length - 1, 0));
  const optionId = (index: number) => `${listId}-${String(index)}`;

  const activeId = expanded ? optionId(activeIndex) : undefined;
  useEffect(() => {
    if (activeId === undefined) {
      return;
    }
    const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
      document.getElementById(activeId);
    element?.scrollIntoView?.({ block: "nearest" });
  }, [activeId]);

  /** The text being typed is now `value`. */
  function typed(value: string) {
    setText(value);
    onQueryChange?.(value);
  }

  /** Add the suggested person `suggestion` and start over. */
  function pick(suggestion: RecipientSuggestion) {
    const known = recipients.some(
      (r) => r.address.toLowerCase() === suggestion.address.toLowerCase(),
    );
    if (!known) {
      onChange([...recipients, { name: suggestion.name ?? null, address: suggestion.address }]);
    }
    typed("");
    setInvalid(null);
    setOpen(false);
  }

  /** Turn `value` into chips; returns whether everything was valid. */
  function commit(value: string): boolean {
    const parsed = parseRecipients(value);
    if (parsed.length === 0) {
      typed("");
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
    typed("");
    setInvalid(null);
    return true;
  }

  /** The list's own keys; whether `event` was one of them. */
  function onListKey(event: KeyboardEvent<HTMLInputElement>): boolean {
    if (event.nativeEvent.isComposing) {
      return false;
    }
    const picked = listed[activeIndex];
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (!expanded) {
        // Closed with Esc: the arrows bring it back.
        if (typing || listed.length === 0 || text.trim() === "") {
          return false;
        }
        setOpen(true);
        setActive(0);
      } else {
        const by = event.key === "ArrowDown" ? 1 : -1;
        setActive((activeIndex + by + listed.length) % listed.length);
      }
      event.preventDefault();
      return true;
    }
    if (!expanded || picked === undefined) {
      return false;
    }
    if (event.key === "Enter" || event.key === "Tab") {
      event.preventDefault();
      pick(picked);
      return true;
    }
    if (event.key === "Escape") {
      // Only the list closes: the composer holding the field stays open.
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      return true;
    }
    return false;
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (suggests && onListKey(event)) {
      return;
    }
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

  const combobox = suggests
    ? {
        role: "combobox",
        "aria-expanded": expanded,
        "aria-controls": expanded ? listId : undefined,
        "aria-activedescendant": activeId,
        "aria-autocomplete": "list" as const,
      }
    : {};

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
              {...combobox}
              onChange={(event) => {
                const value = event.target.value;
                if (/[,;\n]/u.test(value) && value.trim().length > 1) {
                  commit(value);
                } else {
                  typed(value);
                  setInvalid(null);
                  setOpen(true);
                  setActive(0);
                }
              }}
              onKeyDown={onKeyDown}
              onBlur={() => {
                setOpen(false);
                if (text.trim() !== "") {
                  commit(text);
                }
              }}
            />
          </li>
        </ul>
        {trailing}
      </div>
      {expanded ? (
        <div className={styles["popup"]}>
          <div
            id={listId}
            className={styles["list"]}
            role="listbox"
            aria-label={t("recipients.suggestions", { label })}
          >
            {listed.map((suggestion, index) => (
              <div
                key={suggestion.address.toLowerCase()}
                id={optionId(index)}
                className={styles["option"]}
                role="option"
                aria-selected={index === activeIndex}
                tabIndex={-1}
                onMouseDown={(event) => {
                  // Picked before the input's blur, which would close the list.
                  event.preventDefault();
                  pick(suggestion);
                }}
                onMouseMove={() => {
                  if (index !== activeIndex) {
                    setActive(index);
                  }
                }}
              >
                <Avatar name={suggestion.name ?? suggestion.address} size="sm" />
                <span className={styles["person"]}>
                  {suggestion.name === undefined || suggestion.name === null ? null : (
                    <span className={styles["name"]}>{suggestion.name}</span>
                  )}
                  <span className={styles["address"]}>{suggestion.address}</span>
                </span>
              </div>
            ))}
          </div>
          <div className={styles["hints"]} aria-hidden="true">
            <span className={styles["hint"]}>
              <KbdHint keys="arrowup" />
              <KbdHint keys="arrowdown" />
              {t("recipients.hintMove")}
            </span>
            <span className={styles["hint"]}>
              <KbdHint keys="enter" />
              {t("recipients.hintPick")}
            </span>
            <span className={styles["hint"]}>
              <KbdHint keys="escape" />
              {t("recipients.hintClose")}
            </span>
          </div>
        </div>
      ) : null}
      {searching ? (
        <p className={styles["popup"]} role="status">
          <span className={styles["searching"]}>{t("recipients.searching")}</span>
        </p>
      ) : null}
      {invalid === null ? null : (
        <p id={errorId} className={styles["error"]}>
          <Icon name="warning" />
          <span>{t("recipients.invalid", { address: invalid })}</span>
        </p>
      )}
    </div>
  );
}
