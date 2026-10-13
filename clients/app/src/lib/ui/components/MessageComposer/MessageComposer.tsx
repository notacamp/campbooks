import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";

import { Button } from "../Button/Button";
import { Icon, type IconName } from "../Icon/Icon";
import { IconButton } from "../IconButton/IconButton";
import { RecipientField, type Recipient } from "../RecipientField/RecipientField";

import { applyFormat, type Format } from "./formatting";
import styles from "./MessageComposer.module.css";

/** A forwarded attachment, shown with a way to drop it. */
export interface ComposerAttachment {
  id: string;
  filename: string;
  /** Bytes. */
  size: number;
}

export interface MessageComposerProps {
  /** What is being written: "Reply to Maya Okafor", "Forward", "New message". */
  title: string;
  /** Who it is from: the inbox ("Sam Rivers <sam@example.com>"). */
  from: string;
  to: readonly Recipient[];
  onToChange: (to: Recipient[]) => void;
  cc: readonly Recipient[];
  onCcChange: (cc: Recipient[]) => void;
  bcc: readonly Recipient[];
  onBccChange: (bcc: Recipient[]) => void;
  /** Shown for a new message and a forward; a reply keeps its subject. */
  subject?: string | undefined;
  onSubjectChange?: ((subject: string) => void) | undefined;
  body: string;
  onBodyChange: (body: string) => void;
  /** The quoted history, folded away under the text; omit for none. */
  quoted?: string | undefined;
  onQuotedChange?: ((quoted: string) => void) | undefined;
  attachments?: readonly ComposerAttachment[] | undefined;
  onRemoveAttachment?: ((id: string) => void) | undefined;
  /** "Saved", "Saving…": said politely next to Send. */
  status?: ReactNode | undefined;
  /** Why it cannot be sent (shown above the buttons, announced). */
  error?: ReactNode | undefined;
  /** While a send is being handed over: Send is disabled. */
  sending?: boolean | undefined;
  /** Put the cursor in To (a new message, a forward) rather than in the text. */
  focus?: "to" | "body" | undefined;
  onSend: () => void;
  /** Esc: close, keeping the draft. */
  onClose: () => void;
  /** Throw the draft away. */
  onDiscard?: (() => void) | undefined;
}

const FORMATS: { format: Format; icon: IconName; shortcut?: string }[] = [
  { format: "bold", icon: "bold", shortcut: "compose.bold" },
  { format: "italic", icon: "italic", shortcut: "compose.italic" },
  { format: "bullets", icon: "bullets" },
  { format: "numbers", icon: "numbers" },
];

function formatSize(bytes: number, locale: string): string {
  const units = ["byte", "kilobyte", "megabyte"] as const;
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: units[unit],
    unitDisplay: "short",
    maximumFractionDigits: unit === 0 ? 0 : 1,
  }).format(value);
}

/**
 * Writing a message: From, To/Cc/Bcc as chips, the subject, plain text with
 * basic formatting, the quoted history and forwarded attachments. While
 * focus is inside it, ⌘↵ sends, ⌘B and ⌘I format and Esc closes.
 */
export function MessageComposer({
  title,
  from,
  to,
  onToChange,
  cc,
  onCcChange,
  bcc,
  onBccChange,
  subject,
  onSubjectChange,
  body,
  onBodyChange,
  quoted,
  onQuotedChange,
  attachments = [],
  onRemoveAttachment,
  status,
  error,
  sending = false,
  focus = "body",
  onSend,
  onClose,
  onDiscard,
}: MessageComposerProps) {
  const { t, i18n } = useTranslation("ui");
  const id = useId();
  const [active, setActive] = useState(false);
  const [copies, setCopies] = useState(cc.length > 0 || bcc.length > 0);
  const [showQuoted, setShowQuoted] = useState(false);
  const bodyRef = useRef<HTMLTextAreaElement>(null);
  const toRef = useRef<HTMLInputElement>(null);
  // Where the cursor starts, read once: the composer focuses when it opens.
  const startAt = useRef(focus);

  // Where formatting leaves the cursor, applied once the new text is in.
  const selection = useRef<{ start: number; end: number } | null>(null);

  function format(kind: Format) {
    const box = bodyRef.current;
    if (box === null) {
      return;
    }
    const edit = applyFormat(
      { text: body, start: box.selectionStart, end: box.selectionEnd },
      kind,
    );
    selection.current = { start: edit.start, end: edit.end };
    onBodyChange(edit.text);
  }

  // Put the cursor back where formatting left it, in the same commit as the
  // new text, so the next key lands inside the marks.
  useLayoutEffect(() => {
    const box = bodyRef.current;
    if (box !== null && selection.current !== null && box.value === body) {
      box.focus();
      box.setSelectionRange(selection.current.start, selection.current.end);
      selection.current = null;
    }
  }, [body]);

  useShortcut("compose.send", onSend, { enabled: active && !sending });
  useShortcut(
    "compose.bold",
    () => {
      format("bold");
    },
    { enabled: active },
  );
  useShortcut(
    "compose.italic",
    () => {
      format("italic");
    },
    { enabled: active },
  );
  useShortcut("overlay.close", onClose, { enabled: active });

  // Its shortcuts are live while focus is anywhere inside it.
  const sectionRef = useRef<HTMLElement>(null);
  useEffect(() => {
    const section = sectionRef.current;
    if (section === null) {
      return;
    }
    const enter = () => {
      setActive(true);
    };
    const leave = (event: FocusEvent) => {
      if (!(event.relatedTarget instanceof Node) || !section.contains(event.relatedTarget)) {
        setActive(false);
      }
    };
    section.addEventListener("focusin", enter);
    section.addEventListener("focusout", leave);
    return () => {
      section.removeEventListener("focusin", enter);
      section.removeEventListener("focusout", leave);
    };
  }, []);

  // After the listeners above, so the first focus makes the shortcuts live.
  useEffect(() => {
    (startAt.current === "to" ? toRef.current : bodyRef.current)?.focus();
  }, []);

  return (
    <section ref={sectionRef} className={styles["composer"]} aria-labelledby={`${id}-title`}>
      <header className={styles["header"]}>
        <h2 id={`${id}-title`} className={styles["title"]}>
          {title}
        </h2>
        <IconButton
          label={t("composer.close")}
          icon="close"
          shortcut="overlay.close"
          onClick={onClose}
        />
      </header>

      <div className={styles["fields"]}>
        <p className={styles["from"]}>
          <span className={styles["fromLabel"]}>{t("composer.from")}</span>
          <span className={styles["fromValue"]}>{from}</span>
        </p>
        <RecipientField
          label={t("composer.to")}
          recipients={to}
          onChange={onToChange}
          inputRef={toRef}
          trailing={
            copies ? undefined : (
              <Button
                variant="ghost"
                onClick={() => {
                  setCopies(true);
                }}
              >
                {t("composer.copies")}
              </Button>
            )
          }
        />
        {copies ? (
          <>
            <RecipientField label={t("composer.cc")} recipients={cc} onChange={onCcChange} />
            <RecipientField label={t("composer.bcc")} recipients={bcc} onChange={onBccChange} />
          </>
        ) : null}
        {subject === undefined ? null : (
          <div className={styles["subjectRow"]}>
            <label className={styles["fromLabel"]} htmlFor={`${id}-subject`}>
              {t("composer.subject")}
            </label>
            <input
              id={`${id}-subject`}
              className={styles["subject"]}
              value={subject}
              maxLength={998}
              onChange={(event) => {
                onSubjectChange?.(event.target.value);
              }}
            />
          </div>
        )}
      </div>

      <div className={styles["toolbar"]} role="toolbar" aria-label={t("composer.formatting")}>
        {FORMATS.map(({ format: kind, icon, shortcut }) => (
          <IconButton
            key={kind}
            label={t(`composer.format.${kind}`)}
            icon={icon}
            shortcut={shortcut}
            onMouseDown={(event) => {
              // Keep the text's selection.
              event.preventDefault();
            }}
            onClick={() => {
              format(kind);
            }}
          />
        ))}
      </div>

      <label className={styles["hidden"]} htmlFor={`${id}-body`}>
        {t("composer.body")}
      </label>
      <textarea
        id={`${id}-body`}
        ref={bodyRef}
        className={styles["body"]}
        value={body}
        onChange={(event) => {
          onBodyChange(event.target.value);
        }}
      />

      {quoted === undefined || quoted === "" ? null : (
        <div className={styles["quoted"]}>
          <Button
            variant="ghost"
            aria-expanded={showQuoted}
            aria-controls={`${id}-quoted`}
            onClick={() => {
              setShowQuoted(!showQuoted);
            }}
          >
            {showQuoted ? t("composer.hideQuoted") : t("composer.showQuoted")}
          </Button>
          {showQuoted ? (
            <>
              <label className={styles["hidden"]} htmlFor={`${id}-quoted`}>
                {t("composer.quoted")}
              </label>
              <textarea
                id={`${id}-quoted`}
                className={styles["quotedText"]}
                value={quoted}
                onChange={(event) => {
                  onQuotedChange?.(event.target.value);
                }}
              />
            </>
          ) : null}
        </div>
      )}

      {attachments.length === 0 ? null : (
        <ul className={styles["attachments"]} aria-label={t("composer.attachments")}>
          {attachments.map((attachment) => (
            <li key={attachment.id} className={styles["attachment"]}>
              <Icon name="attachment" />
              <span className={styles["attachmentName"]}>{attachment.filename}</span>
              <span className={styles["attachmentSize"]}>
                {formatSize(attachment.size, i18n.language)}
              </span>
              {onRemoveAttachment === undefined ? null : (
                <IconButton
                  label={t("composer.removeAttachment", { name: attachment.filename })}
                  icon="close"
                  onClick={() => {
                    onRemoveAttachment(attachment.id);
                  }}
                />
              )}
            </li>
          ))}
        </ul>
      )}

      {error === undefined || error === null ? null : (
        <p className={styles["error"]} role="alert">
          <Icon name="warning" />
          <span>{error}</span>
        </p>
      )}

      <footer className={styles["footer"]}>
        <span className={styles["status"]} aria-live="polite">
          {status}
        </span>
        {onDiscard === undefined ? null : (
          <Button variant="ghost" onClick={onDiscard}>
            {t("composer.discard")}
          </Button>
        )}
        <Button
          variant="primary"
          icon="send"
          shortcut="compose.send"
          disabled={sending}
          onClick={onSend}
        >
          {t("composer.send")}
        </Button>
      </footer>
    </section>
  );
}
