import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { useShortcut, type ShortcutId } from "~/lib/shortcuts";

import {
  formatSize,
  Button,
  Icon,
  IconButton,
  OverflowActions,
  type IconName,
  type OverflowAction,
} from "@notacamp/ui";
import {
  RecipientField,
  type Recipient,
  type RecipientSuggestion,
} from "../RecipientField/RecipientField";

import { composerExtensions } from "./editor";
import { LinkForm } from "./LinkForm";
import { fromMarks, toMarks, type DocNode } from "./marks";
import styles from "./MessageComposer.module.css";

/** A forwarded attachment, shown with a way to drop it. */
export interface ComposerAttachment {
  id: string;
  filename: string;
  /** Bytes. */
  size: number;
}

/** One of the composer's address fields. */
export type RecipientFieldName = "to" | "cc" | "bcc";

/** Who to suggest in To, Cc and Bcc as the member types, per field. */
export interface RecipientSuggester {
  /** The people to suggest for what `field` holds. */
  suggestions: (field: RecipientFieldName) => readonly RecipientSuggestion[];
  /** Whether `field`'s suggestions are on their way. */
  loading: (field: RecipientFieldName) => boolean;
  /** What is being typed in `field`, as it changes. */
  onQueryChange: (field: RecipientFieldName, text: string) => void;
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
  /** Said beside the quoted history, e.g. that it holds only the start of a long message. */
  quotedNote?: ReactNode | undefined;
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
  /** Suggest people in To, Cc and Bcc as the member types; omit for none. */
  suggest?: RecipientSuggester | undefined;
}

/** Below this width of the composer, the less used formats fold into More. */
const COMPACT_REM = 32;
/** From this width of the window the composer can open as a sheet. */
const WIDE = "(width >= 48rem)";

function subscribeWide(change: () => void): () => void {
  if (typeof globalThis.matchMedia !== "function") {
    return () => {
      // Nothing was listened to.
    };
  }
  const query = globalThis.matchMedia(WIDE);
  query.addEventListener("change", change);
  return () => {
    query.removeEventListener("change", change);
  };
}

const isWide = () =>
  typeof globalThis.matchMedia !== "function" || globalThis.matchMedia(WIDE).matches;

/** Whether the element is narrower than {@link COMPACT_REM}; unknown (not laid out) is not. */
function useCompact(ref: React.RefObject<HTMLElement | null>, placement: unknown): boolean {
  const [compact, setCompact] = useState(false);
  useLayoutEffect(() => {
    const element = ref.current;
    if (element === null || typeof ResizeObserver === "undefined") {
      return;
    }
    const measure = () => {
      const width = element.getBoundingClientRect().width;
      const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
      setCompact(width > 0 && width < COMPACT_REM * rem);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [ref, placement]);
  return compact;
}

interface Link {
  from: number;
  to: number;
  words: string;
  href: string;
}

/**
 * Writing a message: From, To/Cc/Bcc as chips, the subject, the text with
 * its formatting shown (saved as marks), the quoted history and forwarded
 * attachments. While focus is inside it, ⌘↵ sends, the formatting keys
 * (⌘B, ⌘I, ⇧⌘X, ⇧⌘K, ⇧⌘8, ⇧⌘7, ⇧⌘9) format, ⇧⌘E expands it into a sheet over the
 * page and Esc closes it (once an address field's suggestions, given by
 * `suggest`, are closed). The text it hands back is always marks.
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
  quotedNote,
  attachments = [],
  onRemoveAttachment,
  status,
  error,
  sending = false,
  focus = "body",
  onSend,
  onClose,
  onDiscard,
  suggest,
}: MessageComposerProps) {
  const { t, i18n } = useTranslation("ui");
  const id = useId();
  const [active, setActive] = useState(false);
  const [copies, setCopies] = useState(cc.length > 0 || bcc.length > 0);
  // Copies that arrive after it opened (the draft shown again as the server
  // holds it) open the fields too, and they stay open like the member's own.
  if (!copies && (cc.length > 0 || bcc.length > 0)) {
    setCopies(true);
  }
  const [showQuoted, setShowQuoted] = useState(false);
  const [link, setLink] = useState<Link | null>(null);
  const [expanded, setExpanded] = useState(false);
  const wide = useSyncExternalStore(subscribeWide, isWide, () => true);
  const open = expanded && wide;
  const toRef = useRef<HTMLInputElement>(null);
  const frameRef = useRef<HTMLDialogElement>(null);
  const sectionRef = useRef<HTMLElement>(null);
  const compact = useCompact(sectionRef, open);
  // Where the cursor starts, read once: the composer focuses when it opens.
  const startAt = useRef(focus);

  // Each address field's suggestions, when there are any to give.
  const suggestIn = (field: RecipientFieldName) =>
    suggest === undefined
      ? {}
      : {
          suggestions: suggest.suggestions(field),
          loadingSuggestions: suggest.loading(field),
          onQueryChange: (text: string) => {
            suggest.onQueryChange(field, text);
          },
        };

  // The text: one editor for the life of the composer (also while expanded),
  // set from the draft's marks once; saving never moves the cursor.
  const changed = useRef(onBodyChange);
  useEffect(() => {
    changed.current = onBodyChange;
  });
  const editor = useEditor({
    extensions: composerExtensions(),
    content: fromMarks(body),
    editorProps: {
      attributes: {
        id: `${id}-body`,
        role: "textbox",
        "aria-multiline": "true",
        "aria-label": t("composer.body"),
        class: styles["editor"] ?? "",
      },
    },
    onUpdate: ({ editor: updated }) => {
      changed.current(toMarks(updated.getJSON() as DocNode));
    },
  });

  // ⌘↵ in the text sends (TipTap would split the block): the extension calls this.
  useEffect(() => {
    Object.assign((editor.storage as unknown as Record<string, object>)["composerSend"] ?? {}, {
      send: () => {
        if (!sending) {
          onSend();
        }
      },
    });
  }, [editor, onSend, sending]);

  // The server's echo replaces the text only when it differs from what the
  // editor holds, so a save never moves the cursor.
  useEffect(() => {
    if (toMarks(editor.getJSON() as DocNode) === body) {
      return;
    }
    const next = fromMarks(body);
    if (JSON.stringify(next) !== JSON.stringify(editor.getJSON())) {
      editor.commands.setContent(next, { emitUpdate: false });
    }
  }, [editor, body]);

  useEffect(() => {
    editor.view.dom.setAttribute("aria-label", t("composer.body"));
  }, [editor, t]);

  const marks = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      strike: current.isActive("strike"),
      link: current.isActive("link"),
      bullets: current.isActive("bulletList"),
      numbers: current.isActive("orderedList"),
      quote: current.isActive("blockquote"),
    }),
  });

  const run = (command: (chain: ReturnType<typeof editor.chain>) => boolean) => {
    command(editor.chain().focus());
  };
  const format: Record<"bold" | "italic" | "strike" | "bullets" | "numbers" | "quote", () => void> =
    {
      bold: () => {
        run((chain) => chain.toggleBold().run());
      },
      italic: () => {
        run((chain) => chain.toggleItalic().run());
      },
      strike: () => {
        run((chain) => chain.toggleStrike().run());
      },
      bullets: () => {
        run((chain) => chain.toggleBulletList().run());
      },
      numbers: () => {
        run((chain) => chain.toggleOrderedList().run());
      },
      quote: () => {
        run((chain) => chain.toggleBlockquote().run());
      },
    };

  function openLink() {
    if (editor.isActive("link")) {
      editor.chain().extendMarkRange("link").run();
    }
    const { from: start, to: end } = editor.state.selection;
    const href: unknown = editor.isActive("link") ? editor.getAttributes("link")["href"] : "";
    setLink({
      from: start,
      to: end,
      words: editor.state.doc.textBetween(start, end, " "),
      href: typeof href === "string" ? href : "",
    });
  }

  function applyLink(words: string, href: string) {
    if (link !== null) {
      editor
        .chain()
        .focus()
        .insertContentAt(
          { from: link.from, to: link.to },
          { type: "text", text: words, marks: [{ type: "link", attrs: { href } }] },
        )
        .run();
    }
    setLink(null);
  }

  function removeLink() {
    if (link !== null) {
      editor.chain().focus().setTextSelection({ from: link.from, to: link.to }).unsetLink().run();
    }
    setLink(null);
  }

  function closeLink() {
    setLink(null);
    editor.commands.focus();
  }

  const shrink = useCallback(() => {
    setExpanded(false);
  }, []);

  // Expand puts the composer in a modal dialog (the top layer) and Shrink
  // puts it back inline, where it is no dialog at all. Its parts move with
  // it; the editor is one instance for the life of the composer (TipTap
  // keeps its view while it moves), so its cursor and history stay.
  const placed = useRef(false);
  useLayoutEffect(() => {
    const frame = frameRef.current;
    if (open && frame !== null) {
      // jsdom has no showModal: the open attribute does for it.
      if (typeof frame.showModal === "function") {
        frame.showModal();
      } else {
        frame.setAttribute("open", "");
      }
    }
    if (placed.current) {
      editor.commands.focus();
    }
    placed.current = true;
  }, [open, editor]);

  // Esc is the registry's (it shrinks); the browser's own cancel and a click
  // on the scrim (the dialog itself, outside the composer) shrink it too.
  useEffect(() => {
    const frame = frameRef.current;
    if (frame === null) {
      return;
    }
    const cancel = (event: Event) => {
      event.preventDefault();
      shrink();
    };
    const scrim = (event: MouseEvent) => {
      if (event.target === frame) {
        shrink();
      }
    };
    frame.addEventListener("cancel", cancel);
    frame.addEventListener("click", scrim);
    return () => {
      frame.removeEventListener("cancel", cancel);
      frame.removeEventListener("click", scrim);
    };
  }, [shrink, open]);

  useShortcut("compose.send", onSend, { enabled: active && !sending });
  const live = { enabled: active };
  useShortcut("compose.bold", format.bold, live);
  useShortcut("compose.italic", format.italic, live);
  useShortcut("compose.strike", format.strike, live);
  useShortcut("compose.bullets", format.bullets, live);
  useShortcut("compose.numbers", format.numbers, live);
  useShortcut("compose.quote", format.quote, live);
  useShortcut("compose.link", openLink, live);
  useShortcut(
    "compose.expand",
    () => {
      setExpanded(!open);
    },
    { enabled: active && wide },
  );
  useShortcut(
    "overlay.close",
    () => {
      if (link !== null) {
        closeLink();
      } else if (open) {
        shrink();
      } else {
        onClose();
      }
    },
    { enabled: active },
  );

  // Its shortcuts are live while focus is anywhere inside it.
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
  }, [open]);

  // After the listeners above, so the first focus makes the shortcuts live.
  const started = useRef(false);
  useEffect(() => {
    if (started.current) {
      return;
    }
    started.current = true;
    if (startAt.current === "to") {
      toRef.current?.focus();
    } else {
      editor.commands.focus("end");
    }
  }, [editor]);

  const toolbar = (
    name: "bold" | "italic" | "strike" | "link" | "bullets" | "numbers" | "quote",
    shortcut: ShortcutId,
    icon: IconName,
    select: () => void,
  ): OverflowAction => ({
    id: name,
    label: t(`composer.format.${name}`),
    icon,
    shortcut,
    pressed: marks[name],
    onSelect: select,
  });
  const bold = toolbar("bold", "compose.bold", "bold", format.bold);
  const italic = toolbar("italic", "compose.italic", "italic", format.italic);
  const strike = toolbar("strike", "compose.strike", "strike", format.strike);
  const linkAction = toolbar("link", "compose.link", "link", openLink);
  const bullets = toolbar("bullets", "compose.bullets", "bullets", format.bullets);
  const numbers = toolbar("numbers", "compose.numbers", "numbers", format.numbers);
  const quote = toolbar("quote", "compose.quote", "quote", format.quote);

  const composer = (
    <section ref={sectionRef} className={styles["composer"]} aria-labelledby={`${id}-title`}>
      <header className={styles["header"]}>
        <h2 id={`${id}-title`} className={styles["title"]}>
          {title}
        </h2>
        {wide ? (
          <IconButton
            label={open ? t("composer.shrink") : t("composer.expand")}
            icon={open ? "shrink" : "expand"}
            shortcut="compose.expand"
            aria-pressed={open}
            onClick={() => {
              setExpanded(!open);
            }}
          />
        ) : null}
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
          {...suggestIn("to")}
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
            <RecipientField
              label={t("composer.cc")}
              recipients={cc}
              onChange={onCcChange}
              {...suggestIn("cc")}
            />
            <RecipientField
              label={t("composer.bcc")}
              recipients={bcc}
              onChange={onBccChange}
              {...suggestIn("bcc")}
            />
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
        <OverflowActions
          label={t("composer.moreFormatting")}
          actions={
            compact
              ? [bold, italic, linkAction, bullets]
              : [bold, italic, strike, linkAction, bullets, numbers, quote]
          }
          more={compact ? [strike, numbers, quote] : []}
        />
      </div>

      {link === null ? null : (
        <LinkForm
          key={`${String(link.from)}-${String(link.to)}`}
          initialWords={link.words}
          initialAddress={link.href}
          onApply={applyLink}
          onRemove={link.href === "" ? undefined : removeLink}
          onCancel={closeLink}
        />
      )}

      <EditorContent editor={editor} className={styles["text"]} />

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
          {quotedNote === undefined ? null : <p className={styles["quotedNote"]}>{quotedNote}</p>}
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
        {onDiscard === undefined ? null : (
          <IconButton label={t("composer.discard")} icon="trash" onClick={onDiscard} />
        )}
        <span className={styles["status"]} aria-live="polite">
          {status}
        </span>
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

  // Inline it is no dialog; only the expanded sheet is one (modal).
  return open ? (
    <dialog
      ref={frameRef}
      className={styles["frame"]}
      data-expanded
      aria-labelledby={`${id}-title`}
    >
      {composer}
    </dialog>
  ) : (
    <div className={styles["frame"]}>{composer}</div>
  );
}
