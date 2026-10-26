import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";

import { Button, VisuallyHidden, Visor } from "@notacamp/ui";

import styles from "./Composer.module.css";

export interface ComposerProps {
  /** Label and placeholder: "Reply to Sofia". */
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** ⌘↵ while writing. */
  onSend: () => void;
  /** ⌘J while writing: the assistant drafts a reply. */
  onDraft?: (() => void) | undefined;
}

/**
 * The reply box at the end of a conversation. Its shortcuts (⌘↵ send,
 * ⌘J draft) work while you type in it, and only then.
 */
export function Composer({ label, value, onChange, onSend, onDraft }: ComposerProps) {
  const { t } = useTranslation("shortcuts");
  const id = useId();
  const [focused, setFocused] = useState(false);
  useShortcut("compose.send", onSend, { enabled: focused });
  useShortcut(
    "compose.draft",
    () => {
      onDraft?.();
    },
    { enabled: focused && onDraft !== undefined },
  );

  return (
    <div className={styles["composer"]}>
      <label htmlFor={id}>
        <VisuallyHidden>{label}</VisuallyHidden>
      </label>
      <textarea
        id={id}
        className={styles["input"]}
        placeholder={label}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        onFocus={() => {
          setFocused(true);
        }}
        onBlur={() => {
          setFocused(false);
        }}
      />
      <div className={styles["bar"]}>
        {onDraft === undefined ? null : (
          <Button variant="ghost" leading={<Visor />} shortcut="compose.draft" onClick={onDraft}>
            {t("compose.draft")}
          </Button>
        )}
        <span className={styles["spacer"]} />
        <Button variant="primary" shortcut="compose.send" onClick={onSend}>
          {t("compose.send")}
        </Button>
      </div>
    </div>
  );
}
