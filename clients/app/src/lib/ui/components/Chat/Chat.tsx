import {
  useEffect,
  useId,
  useRef,
  useSyncExternalStore,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import {
  cx,
  useFocusTrap,
  Button,
  Icon,
  IconButton,
  Popover,
  VisuallyHidden,
  Visor,
  type VisorState,
} from "@notacamp/ui";

import styles from "./Chat.module.css";

/** From the large breakpoint the panel sits beside the page. */
const WIDE = "(width >= 64rem)";

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
  typeof globalThis.matchMedia === "function" && globalThis.matchMedia(WIDE).matches;

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
  /**
   * Opened from a dialog, which stays open: from the large breakpoint the
   * panel stands over the page at its edge, beside the dialog (which moves
   * aside for it), and takes the focus; Esc closes the panel first.
   */
  over?: boolean | undefined;
  /** In the header, before Close: Recent ({@link ChatRecent}). */
  actions?: ReactNode | undefined;
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
  over = false,
  actions,
}: ChatPanelProps) {
  const titleId = useId();
  const root = useRef<HTMLElement>(null);
  const log = useRef<HTMLDivElement>(null);
  const wide = useSyncExternalStore(subscribeWide, isWide, () => true);
  // Over a dialog on a wide screen: at the page's edge, the dialog beside it.
  const floating = over && modal === undefined && wide;
  const asDialog = floating || (modal ?? !wide);
  useFocusTrap(root, asDialog);
  useEffect(() => {
    if (!floating) {
      return;
    }
    const page = document.documentElement;
    page.dataset["chatBeside"] = "";
    return () => {
      delete page.dataset["chatBeside"];
    };
  }, [floating]);

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
        {actions}
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
      <div
        ref={(element) => {
          root.current = element;
        }}
        className={cx(styles["panel"], floating ? styles["floating"] : styles["sheet"])}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        {body}
      </div>,
      document.body,
    );
  }
  return (
    <section ref={root} className={cx(styles["panel"], styles["aside"])} aria-labelledby={titleId}>
      {body}
    </section>
  );
}

/** One of the member's chats, as Recent lists it. */
export interface ChatRecentItem {
  id: string;
  /** Its first question, the member's own words. */
  text: string;
  /** When its last message was written, in words ("Yesterday", "09:40"). */
  time: string;
  /** The same, for machines. */
  dateTime: string;
}

export interface ChatRecentProps {
  /** The button's and the menu's name: "Recent". */
  label: string;
  /** The chats, newest first; `undefined` while they load. */
  items: readonly ChatRecentItem[] | undefined;
  /** Said when there is none: "No chats yet". */
  empty: string;
  /** Said while they load. */
  loading: string;
  /** Said when they could not be listed. */
  failed?: string | undefined;
  /** The chat on screen, marked as current. */
  current?: string | undefined;
  /** The menu opened: list them afresh. */
  onOpen?: (() => void) | undefined;
  /** Open this chat. */
  onPick: (id: string) => void;
}

/**
 * "Recent", in the panel's header: the member's chats by their first
 * question and the time of their last message; choosing one opens it. ↑ ↓
 * move between them, Esc closes the menu (and only the menu).
 */
export function ChatRecent({
  label,
  items,
  empty,
  loading,
  failed,
  current,
  onOpen,
  onPick,
}: ChatRecentProps) {
  return (
    <Popover
      label={label}
      onOpenChange={(open) => {
        if (open) {
          onOpen?.();
        }
      }}
      renderTrigger={(trigger) => <IconButton {...trigger} label={label} icon="clock" />}
    >
      {(close) => (
        <div className={styles["recent"]}>
          {failed === undefined ? null : <p className={styles["recentNote"]}>{failed}</p>}
          {failed === undefined && items === undefined ? (
            <p className={styles["recentNote"]}>{loading}</p>
          ) : null}
          {items?.length === 0 ? <p className={styles["recentNote"]}>{empty}</p> : null}
          {items?.map((item) => (
            <button
              key={item.id}
              type="button"
              className={styles["recentItem"]}
              aria-current={item.id === current ? "true" : undefined}
              onKeyDown={moveBetweenRows}
              onClick={() => {
                close();
                onPick(item.id);
              }}
            >
              <span className={styles["recentText"]}>{item.text}</span>
              <time className={styles["recentTime"]} dateTime={item.dateTime}>
                {item.time}
              </time>
            </button>
          ))}
        </div>
      )}
    </Popover>
  );
}

/** ↑/↓ move between a menu's rows (the row's sibling buttons). */
function moveBetweenRows(event: ReactKeyboardEvent<HTMLButtonElement>) {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
    return;
  }
  const row = event.currentTarget;
  const rows = [
    ...(row.parentElement?.querySelectorAll<HTMLButtonElement>(":scope > button") ?? []),
  ];
  const at = rows.indexOf(row);
  const next = rows[(at + (event.key === "ArrowDown" ? 1 : rows.length - 1)) % rows.length];
  event.preventDefault();
  event.stopPropagation();
  next?.focus();
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

/** Where a proposal stands, as its card shows it. */
export type ChatProposalState =
  "pending" | "working" | "done" | "undone" | "declined" | "failed" | "blocked";

export interface ChatProposalProps {
  /** Who proposes it, above the card: "Scout proposes". */
  label: string;
  /** The change, as an action: "Archive", "Move to Updates", "Later, until Monday 9:00". */
  title: string;
  /** What it is about: "Marta Ferreira · Walkthrough on Thursday". */
  about?: string | undefined;
  /**
   * Scout's own words, shown quoted as plain text: a label name, a rule
   * sentence, a reply intent. Nothing else on the card is the model's.
   */
  quote?: string | undefined;
  /** Who the quote is from: "Scout’s words". */
  quoteLabel?: string | undefined;
  /**
   * What the member is confirming, shown in full under the title: a section
   * as the page would show it, who will read it. Anything the card must show
   * before Confirm belongs here, not behind a link.
   */
  body?: ReactNode | undefined;
  /** Where it stands; `pending` is the only state with a choice to make. */
  state: ChatProposalState;
  /** The state as a sentence once it is not pending: "Done.", "Not now.". */
  status?: string | undefined;
  /** Confirm and Not now while pending; Undo once done (one button style). */
  actions?: ReactNode | undefined;
}

/**
 * A change Scout proposes under its answer: what, about what, Scout's own
 * words quoted, and the member's choice. Nothing happens until they choose;
 * once settled the card says how it ended, and a settled card read again
 * has no buttons.
 */
export function ChatProposal({
  label,
  title,
  about,
  quote,
  quoteLabel,
  body,
  state,
  status,
  actions,
}: ChatProposalProps) {
  const titleId = useId();
  const settled = state !== "pending" && state !== "working";
  return (
    <section className={styles["proposal"]} data-state={state} aria-labelledby={titleId}>
      <p className={styles["proposalLabel"]}>
        <Visor state={state === "working" ? "thinking" : "idle"} size="xs" />
        <span>{label}</span>
      </p>
      <h3 className={styles["proposalTitle"]} id={titleId}>
        {title}
      </h3>
      {about === undefined ? null : <p className={styles["proposalAbout"]}>{about}</p>}
      {body === undefined ? null : <div className={styles["proposalBody"]}>{body}</div>}
      {quote === undefined ? null : (
        <figure className={styles["proposalQuote"]}>
          <blockquote className={styles["proposalQuoteText"]}>
            <q>{quote}</q>
          </blockquote>
          {quoteLabel === undefined ? null : (
            <figcaption className={styles["proposalAttribution"]}>{quoteLabel}</figcaption>
          )}
        </figure>
      )}
      <p className={styles["proposalStatus"]} role="status">
        {settled || state === "working" ? (
          <>
            {state === "done" ? <Icon name="check" /> : null}
            <span>{status}</span>
          </>
        ) : null}
      </p>
      {actions === undefined ? null : <div className={styles["proposalActions"]}>{actions}</div>}
    </section>
  );
}
