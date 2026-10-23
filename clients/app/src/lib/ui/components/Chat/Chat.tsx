import { useEffect, useId, useRef, useSyncExternalStore, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cx } from "../../internal/cx";
import { useFocusTrap } from "../../internal/use-focus-trap";
import { Button } from "../Button/Button";
import { Icon } from "../Icon/Icon";
import { IconButton } from "../IconButton/IconButton";
import { VisuallyHidden } from "../Typography/Typography";
import { Visor, type VisorState } from "../Visor/Visor";

import styles from "./Chat.module.css";

/** From the large breakpoint the panel sits beside the page. */
const WIDE = "(width >= 64rem)";

function subscribeWide(change: () => void): () => void {
  const query = globalThis.matchMedia?.(WIDE);
  query?.addEventListener("change", change);
  return () => {
    query?.removeEventListener("change", change);
  };
}

function isWide(): boolean {
  return globalThis.matchMedia?.(WIDE).matches ?? false;
}

export interface ChatPanelProps {
  /** Its title and accessible name: "Scout". */
  title: string;
  /** The visor next to the title. */
  visor?: VisorState | undefined;
  /** Close's label: "Close". */
  closeLabel: string;
  /** Close, or Esc: the opener takes the focus back. */
  onClose: () => void;
  /** Under the title: what the chat is about ({@link ChatAbout}). */
  about?: ReactNode | undefined;
  /** The messages, oldest first. */
  children: ReactNode;
  /** The field and its note, or what is said in its place. */
  footer: ReactNode;
  /** Scout's finished answer, announced once in a polite live region. */
  announcement?: string | undefined;
  /** Changes whenever the conversation grows: the log follows its end. */
  follow?: unknown;
  /**
   * Over the whole screen, as a dialog that traps focus (below the large
   * breakpoint), instead of beside the page as a labelled region. Follows
   * the window when absent.
   */
  modal?: boolean | undefined;
}

/**
 * A chat beside the page (from the large breakpoint, the page keeping the
 * rest of the width) or over the whole screen below it, as a dialog that
 * traps focus. Esc closes it from anywhere inside. The log follows its end
 * as it grows; a finished answer is announced politely, once.
 */
export function ChatPanel({
  title,
  visor = "idle",
  closeLabel,
  onClose,
  about,
  children,
  footer,
  announcement,
  follow,
  modal,
}: ChatPanelProps) {
  const titleId = useId();
  const root = useRef<HTMLElement>(null);
  const log = useRef<HTMLDivElement>(null);
  const wide = useSyncExternalStore(subscribeWide, isWide, () => true);
  const asDialog = modal ?? !wide;
  useFocusTrap(root, asDialog);

  useEffect(() => {
    const element = log.current;
    if (element !== null) {
      element.scrollTop = element.scrollHeight;
    }
  }, [follow]);

  // Esc closes it from anywhere inside, and only it: it stops here so the
  // page's shortcuts never see it.
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    const element = root.current;
    if (element === null) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        close.current();
      }
    };
    element.addEventListener("keydown", onKeyDown);
    return () => {
      element.removeEventListener("keydown", onKeyDown);
    };
  }, [asDialog]);

  const body = (
    <>
      <header className={styles["header"]}>
        <Visor state={visor} size="md" />
        <h2 className={styles["title"]} id={titleId}>
          {title}
        </h2>
        <span className={styles["spacer"]} />
        <IconButton icon="close" label={closeLabel} shortcut="overlay.close" onClick={onClose} />
      </header>
      {about === undefined ? null : <div className={styles["about"]}>{about}</div>}
      <div ref={log} className={styles["log"]}>
        {children}
      </div>
      <div className={styles["footer"]}>{footer}</div>
      <VisuallyHidden>
        <span aria-live="polite">{announcement ?? ""}</span>
      </VisuallyHidden>
    </>
  );

  if (asDialog) {
    return createPortal(
      <section
        ref={root}
        className={cx(styles["panel"], styles["sheet"])}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        {body}
      </section>,
      document.body,
    );
  }
  return (
    <section ref={root} className={cx(styles["panel"], styles["aside"])} aria-labelledby={titleId}>
      {body}
    </section>
  );
}

export interface ChatAboutProps {
  /** "About". */
  label: string;
  /** Who and what: "Marta Silva · Rent for November". */
  children: ReactNode;
  /** Open what it is about. */
  action?: ReactNode | undefined;
}

/** What a chat is about, in one line, with a way to open it. */
export function ChatAbout({ label, children, action }: ChatAboutProps) {
  return (
    <p className={styles["aboutLine"]}>
      <span className={styles["aboutLabel"]}>{label}</span>
      <span className={styles["aboutText"]}>{children}</span>
      {action}
    </p>
  );
}

/** Blocks of plain text: paragraphs, and runs of lines starting with "- " as lists. */
type Block = { kind: "paragraph"; text: string } | { kind: "list"; items: string[] };

function blocksOf(text: string): Block[] {
  const blocks: Block[] = [];
  for (const chunk of text.split(/\n\s*\n/u)) {
    const lines = chunk.split("\n").filter((line) => line.trim() !== "");
    let paragraph: string[] = [];
    let list: string[] = [];
    const flush = () => {
      if (paragraph.length > 0) {
        blocks.push({ kind: "paragraph", text: paragraph.join("\n") });
        paragraph = [];
      }
      if (list.length > 0) {
        blocks.push({ kind: "list", items: list });
        list = [];
      }
    };
    for (const line of lines) {
      const item = /^\s*- (.*)$/u.exec(line);
      if (item === null) {
        if (list.length > 0) {
          flush();
        }
        paragraph.push(line);
      } else {
        if (paragraph.length > 0) {
          flush();
        }
        list.push(item[1] ?? "");
      }
    }
    flush();
  }
  return blocks;
}

/**
 * Text written by a model, as plain text only: paragraphs, and lines
 * starting with "- " as a list. Never HTML, never turned into links.
 */
export function ChatText({ text }: { text: string }) {
  return (
    <div className={styles["text"]}>
      {blocksOf(text).map((block, index) =>
        block.kind === "paragraph" ? (
          <p key={index}>{block.text}</p>
        ) : (
          <ul key={index}>
            {block.items.map((item, at) => (
              <li key={at}>{item}</li>
            ))}
          </ul>
        ),
      )}
    </div>
  );
}

export interface ChatMessageProps {
  /** Who wrote it. */
  from: "member" | "scout";
  /** Its accessible name: "You asked", "Scout answered". */
  label: string;
  /** Its words, plain text; empty while Scout has written none yet. */
  text: string;
  /** Before the words: Scout's thinking, its steps as they run. */
  before?: ReactNode | undefined;
  /** After the words: what Scout looked at, a failure and Try again. */
  after?: ReactNode | undefined;
  /** Scout is still writing it. */
  writing?: boolean | undefined;
}

/** One question or answer of a chat. */
export function ChatMessage({
  from,
  label,
  text,
  before,
  after,
  writing = false,
}: ChatMessageProps) {
  return (
    <article
      className={cx(styles["message"], from === "member" && styles["member"])}
      aria-label={label}
      aria-busy={writing || undefined}
    >
      {before}
      {text === "" ? null : <ChatText text={text} />}
      {after}
    </article>
  );
}

export interface ChatStepRecord {
  /** Stable key. */
  id: string;
  /** What it is: "Marta Silva · Rent for November". */
  label: string;
  onOpen: () => void;
}

export interface ChatStep {
  /** The step as a sentence: "Read what happened to this email". */
  text: string;
  /** What it found, each opening on activation. */
  records?: readonly ChatStepRecord[] | undefined;
}

export interface ChatStepsProps {
  /** The disclosure's summary: "What I looked at". */
  summary: string;
  steps: readonly ChatStep[];
}

/** What Scout looked at to answer, one click away; each record it found opens. */
export function ChatSteps({ summary, steps }: ChatStepsProps) {
  if (steps.length === 0) {
    return null;
  }
  return (
    <details className={styles["steps"]}>
      <summary className={styles["stepsSummary"]}>
        <Icon name="chevron-down" className={styles["chevron"]} />
        <span>{summary}</span>
      </summary>
      <ul className={styles["stepList"]}>
        {steps.map((step, index) => (
          <li key={index}>
            <span>{step.text}</span>
            {step.records === undefined || step.records.length === 0 ? null : (
              <span className={styles["records"]}>
                {step.records.map((record) => (
                  <Button key={record.id} variant="link" onClick={record.onOpen}>
                    {record.label}
                  </Button>
                ))}
              </span>
            )}
          </li>
        ))}
      </ul>
    </details>
  );
}

export interface ChatWorkingProps {
  /** What is happening, newest last: "Reading what happened to this email…". */
  lines: readonly string[];
}

/** Scout at work: a line per step as it runs, the newest with the visor thinking. */
export function ChatWorking({ lines }: ChatWorkingProps) {
  if (lines.length === 0) {
    return null;
  }
  return (
    <ul className={styles["working"]}>
      {lines.map((line, index) => (
        <li key={index} className={styles["workingLine"]}>
          {index === lines.length - 1 ? (
            <Visor state="thinking" size="xs" />
          ) : (
            <Icon name="check" />
          )}
          <span>{line}</span>
        </li>
      ))}
    </ul>
  );
}

export interface ChatSuggestionsProps {
  /** Accessible name of the group: "Questions to start from". */
  label: string;
  /** Scout's opening line, above them. */
  intro: string;
  questions: readonly string[];
  onPick: (question: string) => void;
}

/** A chat's opening line and questions to tap, each asked as if typed. */
export function ChatSuggestions({ label, intro, questions, onPick }: ChatSuggestionsProps) {
  return (
    <div className={styles["suggestions"]}>
      <p className={styles["intro"]}>
        <Visor size="xs" />
        <span>{intro}</span>
      </p>
      <div className={styles["questions"]} role="group" aria-label={label}>
        {questions.map((question) => (
          <Button
            key={question}
            variant="secondary"
            onClick={() => {
              onPick(question);
            }}
          >
            {question}
          </Button>
        ))}
      </div>
    </div>
  );
}

export interface ChatFieldProps {
  /** The field's label and placeholder: "Ask about this". */
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** Ask (Enter; Shift+Enter is a new line). */
  onAsk: () => void;
  /** Ask's label. */
  askLabel: string;
  /** Scout is answering: the field stays usable, Ask waits. */
  busy?: boolean | undefined;
  /** The line under it: "Scout looks things up and explains…". */
  note?: string | undefined;
}

/**
 * Where the member asks: Enter asks, Shift+Enter starts a new line. The
 * field stays usable while Scout answers; Ask waits until it is done.
 */
export function ChatField({
  label,
  value,
  onChange,
  onAsk,
  askLabel,
  busy = false,
  note,
}: ChatFieldProps) {
  const id = useId();
  const noteId = useId();
  const ready = !busy && value.trim() !== "";
  return (
    <div className={styles["field"]}>
      <div className={styles["box"]}>
        <label htmlFor={id}>
          <VisuallyHidden>{label}</VisuallyHidden>
        </label>
        <textarea
          id={id}
          data-autofocus
          className={styles["input"]}
          placeholder={label}
          rows={2}
          value={value}
          aria-describedby={note === undefined ? undefined : noteId}
          onChange={(event) => {
            onChange(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              if (ready) {
                onAsk();
              }
            }
          }}
        />
        <Button variant="primary" shortcut="chat.send" disabled={!ready} onClick={onAsk}>
          {askLabel}
        </Button>
      </div>
      {note === undefined ? null : (
        <p className={styles["note"]} id={noteId}>
          {note}
        </p>
      )}
    </div>
  );
}

export interface ChatNoticeProps {
  /** What is said in place of the field or under an answer. */
  children: ReactNode;
  /** The visor's state: `dizzy` when something failed, `asleep` when Scout can't. */
  visor?: VisorState | undefined;
  /** One action: Try again, Start over. */
  action?: ReactNode | undefined;
}

/** A line from Scout about the chat itself, with at most one action. */
export function ChatNotice({ children, visor = "unsure", action }: ChatNoticeProps) {
  return (
    <div className={styles["notice"]}>
      <Visor state={visor} size="xs" />
      <span className={styles["noticeText"]}>{children}</span>
      {action}
    </div>
  );
}
