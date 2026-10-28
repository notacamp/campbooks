import { useEffect, useId, useRef, useState, type SyntheticEvent } from "react";
import { useTranslation } from "react-i18next";

import { Button, TextField } from "@notacamp/ui";

import { isLinkable, normalizeAddress } from "./marks";
import styles from "./MessageComposer.module.css";

export interface LinkFormProps {
  /** The selected words, if any. */
  initialWords: string;
  /** The link's address when editing one. */
  initialAddress: string;
  /** Link the words to the address (only a web or mail address gets here). */
  onApply: (words: string, address: string) => void;
  /** Take the link off the words (editing a link). */
  onRemove?: (() => void) | undefined;
  onCancel: () => void;
}

/**
 * Link: the words and the address, in a row under the toolbar (not a modal,
 * so it works in the expanded sheet too). Only web and mail addresses are
 * taken; the server links no other.
 */
export function LinkForm({
  initialWords,
  initialAddress,
  onApply,
  onRemove,
  onCancel,
}: LinkFormProps) {
  const { t } = useTranslation("ui");
  const id = useId();
  const [words, setWords] = useState(initialWords);
  const [address, setAddress] = useState(initialAddress);
  const [refused, setRefused] = useState(false);
  // The field to fill in first: the words when none are selected, else the address.
  const first = useRef<HTMLInputElement>(null);
  useEffect(() => {
    first.current?.focus();
  }, []);

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    event.stopPropagation();
    const href = normalizeAddress(address);
    if (!isLinkable(href)) {
      setRefused(true);
      return;
    }
    onApply(words.trim() === "" ? address.trim() : words, href);
  }

  return (
    <form className={styles["link"]} aria-labelledby={`${id}-title`} onSubmit={submit}>
      <p id={`${id}-title`} className={styles["linkTitle"]}>
        {t("composer.link.title")}
      </p>
      <TextField
        label={t("composer.link.words")}
        value={words}
        ref={initialWords === "" ? first : undefined}
        onChange={(event) => {
          setWords(event.target.value);
        }}
      />
      <TextField
        label={t("composer.link.address")}
        value={address}
        type="text"
        inputMode="url"
        autoComplete="off"
        ref={initialWords === "" ? undefined : first}
        error={refused ? t("composer.link.refused") : undefined}
        onChange={(event) => {
          setAddress(event.target.value);
          setRefused(false);
        }}
      />
      <div className={styles["linkActions"]}>
        {onRemove === undefined ? null : (
          <Button variant="ghost" onClick={onRemove}>
            {t("composer.link.remove")}
          </Button>
        )}
        <Button variant="ghost" onClick={onCancel}>
          {t("composer.link.cancel")}
        </Button>
        <Button variant="primary" type="submit">
          {t(onRemove === undefined ? "composer.link.add" : "composer.link.update")}
        </Button>
      </div>
    </form>
  );
}
