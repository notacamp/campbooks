import { useEffect, useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";

import { isTypingTarget } from "../../shortcuts";

import { useFocusTrap } from "../../internal/use-focus-trap";
import { Button } from "../Button/Button";
import { KbdHint } from "../Kbd/Kbd";
import { Visor, type VisorState } from "../Visor/Visor";

import styles from "./Dialog.module.css";

/** How long a result stays before the dialog closes itself (`--duration-result`). */
const DIALOG_RESULT_MS = 1100;

export interface DialogResult {
  /** What happened, as a sentence: "Snoozed until Monday 08:00". */
  title: string;
  /** One more line, if it helps. */
  detail?: string | undefined;
}

export interface DialogProps {
  open: boolean;
  /** Esc, a click on the scrim and Close: nothing changed. */
  onClose: () => void;
  /** The one decision the dialog asks for: "Snooze Marta Ferreira until…". */
  title: string;
  /** One line under the title. */
  description?: string | undefined;
  /** The visor before the title; `null` for none. */
  visor?: VisorState | null | undefined;
  /** The body: {@link DialogChoices}, a field, a preview. */
  children?: ReactNode;
  /** Buttons at the bottom (Save, Cancel). */
  footer?: ReactNode;
  /**
   * One icon button in the header, before Close (Ask Scout): what it opens
   * leaves the dialog as it is.
   */
  headerAction?: ReactNode;
  /**
   * Once set, the dialog shows the result (visor happy, the sentence and a
   * thin draining bar) and calls `onResultEnd` after about a second, or at
   * once on any key or click.
   */
  result?: DialogResult | undefined;
  onResultEnd?: (() => void) | undefined;
}

/**
 * A modal dialog for one decision: focus stays inside, Esc or a click on
 * the scrim closes it with nothing changed, number keys pick a choice and
 * the arrows move between choices. Below 48rem it is a bottom sheet. Page
 * shortcuts never fire behind it: the focus stays in it, even when the
 * focused control goes away, and plain keys stop at the dialog (⌘/Ctrl
 * combinations still reach the registry). Rendered into <body>.
 */
export function Dialog({
  open,
  onClose,
  title,
  description,
  visor = "idle",
  children,
  footer,
  headerAction,
  result,
  onResultEnd,
}: DialogProps) {
  const { t } = useTranslation("ui");
  const titleId = useId();
  const descriptionId = useId();
  const scrim = useRef<HTMLDivElement>(null);
  const dialog = useRef<HTMLDivElement>(null);
  const latest = useRef({ onClose, onResultEnd, showingResult: result !== undefined });
  useEffect(() => {
    latest.current = { onClose, onResultEnd, showingResult: result !== undefined };
  });
  useFocusTrap(dialog, open);

  useEffect(() => {
    const scrimElement = scrim.current;
    const dialogElement = dialog.current;
    if (!open || scrimElement === null || dialogElement === null) {
      return;
    }
    const onScrimDown = (event: MouseEvent) => {
      if (latest.current.showingResult) {
        latest.current.onResultEnd?.();
      } else if (event.target === scrimElement) {
        latest.current.onClose();
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      const plain = !event.metaKey && !event.ctrlKey && !event.altKey;
      if (latest.current.showingResult) {
        // Any key closes the result sooner, and does nothing else.
        if (event.key !== "Tab") {
          event.preventDefault();
          event.stopPropagation();
          latest.current.onResultEnd?.();
        }
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        latest.current.onClose();
        return;
      }
      if (plain && !isTypingTarget(event.target)) {
        const choices = [...dialogElement.querySelectorAll<HTMLElement>("[data-dialog-choice]")];
        if (/^[1-9]$/u.test(event.key)) {
          const choice = choices[Number(event.key) - 1];
          if (choice !== undefined) {
            event.preventDefault();
            choice.click();
          }
        } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          const at = choices.indexOf(document.activeElement as HTMLElement);
          if (choices.length > 0) {
            event.preventDefault();
            const step = event.key === "ArrowDown" ? 1 : -1;
            const next = at === -1 ? (step === 1 ? 0 : choices.length - 1) : at + step;
            choices[(next + choices.length) % choices.length]?.focus();
          }
        }
      }
      // Page shortcuts never fire behind the dialog.
      if (plain) {
        event.stopPropagation();
      }
    };
    // The focused control went away (a step replaced it), or the focus
    // left for nowhere: it comes back to the dialog, so its keys (Esc, the
    // numbers, the arrows) stay its own and plain keys still stop at it. Only
    // the dialog is watched, never the whole document: a key the page itself
    // receives is the page's (campbooks-cloud#220).
    const reclaim = () => {
      const active = document.activeElement;
      if (active === null || active === document.body || active === document.documentElement) {
        dialogElement.focus();
      }
    };
    const onFocusOut = (event: FocusEvent) => {
      if (event.relatedTarget === null) {
        queueMicrotask(reclaim);
      }
    };
    const replaced = new MutationObserver(reclaim);
    replaced.observe(dialogElement, { childList: true, subtree: true });
    scrimElement.addEventListener("mousedown", onScrimDown);
    dialogElement.addEventListener("keydown", onKeyDown);
    dialogElement.addEventListener("focusout", onFocusOut);
    return () => {
      replaced.disconnect();
      scrimElement.removeEventListener("mousedown", onScrimDown);
      dialogElement.removeEventListener("keydown", onKeyDown);
      dialogElement.removeEventListener("focusout", onFocusOut);
    };
  }, [open]);

  // The result closes itself; the focus waits on the dialog meanwhile.
  const showingResult = open && result !== undefined;
  useEffect(() => {
    if (!showingResult) {
      return;
    }
    dialog.current?.focus();
    const timer = setTimeout(() => {
      latest.current.onResultEnd?.();
    }, DIALOG_RESULT_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [showingResult]);

  if (!open) {
    return null;
  }
  return createPortal(
    <div ref={scrim} className={styles["scrim"]}>
      <div
        ref={dialog}
        className={styles["dialog"]}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={
          description === undefined || result !== undefined ? undefined : descriptionId
        }
        data-result={result === undefined ? undefined : true}
        tabIndex={-1}
      >
        {result === undefined ? (
          <>
            <div className={styles["head"]}>
              {visor === null ? null : <Visor state={visor} />}
              <div className={styles["heading"]}>
                <h2 id={titleId} className={styles["title"]}>
                  {title}
                </h2>
                {description === undefined ? null : (
                  <p id={descriptionId} className={styles["description"]}>
                    {description}
                  </p>
                )}
              </div>
              {headerAction}
              <Button variant="ghost" shortcut="overlay.close" onClick={onClose}>
                {t("close")}
              </Button>
            </div>
            {children === undefined ? null : <div className={styles["body"]}>{children}</div>}
            {footer === undefined ? null : <div className={styles["footer"]}>{footer}</div>}
          </>
        ) : (
          <div className={styles["result"]} role="status">
            <Visor state="happy" size="md" />
            <div className={styles["heading"]}>
              <h2 id={titleId} className={styles["title"]}>
                {result.title}
              </h2>
              {result.detail === undefined ? null : (
                <p className={styles["description"]}>{result.detail}</p>
              )}
            </div>
            <span className={styles["drain"]} aria-hidden="true" />
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}

export interface DialogChoice {
  id: string;
  label: string;
  /** Fine print after the label: "Monday 08:00". */
  detail?: string | undefined;
  /** The choice runs the AI (the small visor before its label). */
  ai?: boolean | undefined;
}

export interface DialogChoicesProps {
  /** In order: the first takes 1, the second 2, up to 9. */
  choices: readonly DialogChoice[];
  onChoose: (id: string) => void;
  /** Accessible name of the group. */
  label?: string | undefined;
  /**
   * Take the focus when shown: for choices that arrive after the dialog
   * opened (the first gets it, as when the dialog opens on them).
   */
  focusOnShow?: boolean | undefined;
}

/** A dialog's choices, one per line, each with its number key. The first gets the focus. */
export function DialogChoices({
  choices,
  onChoose,
  label,
  focusOnShow = false,
}: DialogChoicesProps) {
  const group = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Never away from someone typing (a digit would then pick a choice).
    if (focusOnShow && !isTypingTarget(document.activeElement)) {
      group.current?.querySelector<HTMLElement>("[data-dialog-choice]")?.focus();
    }
  }, [focusOnShow]);
  return (
    <div ref={group} className={styles["choices"]} role="group" aria-label={label}>
      {choices.map((choice, index) => (
        <button
          key={choice.id}
          type="button"
          className={styles["choice"]}
          data-dialog-choice=""
          data-autofocus={index === 0 ? "" : undefined}
          aria-keyshortcuts={index < 9 ? String(index + 1) : undefined}
          onClick={() => {
            onChoose(choice.id);
          }}
        >
          {choice.ai === true ? <Visor size="xs" /> : null}
          <span className={styles["choiceLabel"]}>{choice.label}</span>
          {choice.detail === undefined ? null : (
            <span className={styles["choiceDetail"]}>{choice.detail}</span>
          )}
          {index < 9 ? <KbdHint keys={String(index + 1)} className={styles["choiceKey"]} /> : null}
        </button>
      ))}
    </div>
  );
}
