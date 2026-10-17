import {
  createContext,
  use,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";

import { Button } from "../Button/Button";
import { IconButton } from "../IconButton/IconButton";
import { Visor } from "../Visor/Visor";

import styles from "./Toast.module.css";

export interface ToastProps {
  children: ReactNode;
  /** One action, usually Undo. */
  action?: ReactNode | undefined;
  /**
   * `pending` while an action takes a moment ("Sending to Marta…", the visor
   * thinking); `alert` for failures that need the user (a send failed):
   * wraps, and is announced assertively.
   */
  tone?: "default" | "pending" | "alert" | undefined;
  /** A way to close it (a failure stays until Retry or this). */
  onDismiss?: (() => void) | undefined;
}

const VISOR = { default: "happy", pending: "thinking", alert: "alert" } as const;

/** A short confirmation pill. Put it in a {@link ToastRegion} so it is announced. */
export function Toast({ children, action, tone = "default", onDismiss }: ToastProps) {
  const { t } = useTranslation("ui");
  return (
    <div className={styles["toast"]} data-tone={tone} role={tone === "alert" ? "alert" : undefined}>
      <Visor state={VISOR[tone]} />
      <span className={styles["message"]}>{children}</span>
      {action}
      {onDismiss === undefined ? null : (
        <IconButton icon="close" label={t("toast.dismiss")} onClick={onDismiss} />
      )}
    </div>
  );
}

/** The fixed, polite live region toasts appear in (bottom centre, above a tab bar). */
export function ToastRegion({ children, label }: { children?: ReactNode; label?: string }) {
  const { t } = useTranslation("ui");
  return (
    <div className={styles["region"]} role="status" aria-label={label ?? t("toast.region")}>
      {children}
    </div>
  );
}

export interface UndoableAction {
  /** What happened, as the toast says it: "Done · Sofia Pessoa". */
  label: string;
  undo: () => void;
  /** Lets the history offer Redo after an undo. */
  redo?: (() => void) | undefined;
  /** How long its toast stays up, when the action has a window of its own (a send). */
  durationMs?: number | undefined;
}

export interface PendingAction {
  /** What is happening, as the toast says it: "Sending to Marta…". */
  label: string;
  /**
   * Take it back while it is still pending (Undo, Z); omit when it can't be.
   * It must really stop the action: once cancelled, a later `done` or `fail`
   * is ignored, so the toast keeps saying it stopped.
   */
  cancel?: (() => void) | undefined;
  /** What the toast says once cancelled (default "Stopped: <label>"). */
  cancelled?: string | undefined;
}

export interface PendingFailure {
  /** What went wrong and what is kept: "Couldn't send to Marta. Your draft is kept." */
  label: string;
  /** Try again (the toast's Retry). */
  retry?: (() => void) | undefined;
}

export interface PendingHandle {
  /** It went through: an undoable action (pushed to the history) or just a sentence. */
  done: (result: UndoableAction | { label: string }) => void;
  /** It failed: a failure that stays until Retry or dismiss. */
  fail: (failure: PendingFailure) => void;
}

export interface UndoEntry extends UndoableAction {
  id: number;
  undone: boolean;
}

export interface UndoApi {
  /** Records an action that just happened and shows it with Undo; returns its id. */
  push: (action: UndoableAction) => number;
  /** Takes an entry out of the history: its action failed, so there is nothing to undo. */
  retract: (id: number) => void;
  /** Undoes the most recent action that is not undone yet (Z). */
  undo: () => void;
  /** Undo or redo one entry of the history. */
  toggle: (id: number) => void;
  /** Say something without Undo (that an undo came too late, say). */
  notify: (text: string) => void;
  /** An action that takes a moment: says so at once, then how it went. */
  pending: (action: PendingAction) => PendingHandle;
  /** A failure that stays (announced as an alert) until Retry or dismiss. */
  fail: (failure: PendingFailure) => void;
  /** Newest first, for the session. */
  history: readonly UndoEntry[];
}

const UndoContext = createContext<UndoApi | null>(null);

interface ToastState {
  key: number;
  text: string;
  kind: "done" | "undoable" | "pending" | "failed";
  /** Its own duration, if not the provider's. */
  ms?: number | undefined;
  /** Pending: take it back. */
  cancel?: (() => void) | undefined;
  /** Failed: try again. */
  retry?: (() => void) | undefined;
}

type ShowOptions = Pick<ToastState, "ms" | "cancel" | "retry">;

export interface UndoProviderProps {
  children: ReactNode;
  /** How long a toast stays up. The history keeps every action for the session. */
  toastMs?: number | undefined;
}

/**
 * The undo stack. Every action that changes something pushes an entry; the
 * toast offers Undo, Z undoes the latest, and the history stays available
 * (newest first) for "What Campbooks did"-style lists.
 */
export function UndoProvider({ children, toastMs = 6000 }: UndoProviderProps) {
  const { t } = useTranslation("ui");
  const [history, setHistory] = useState<UndoEntry[]>([]);
  const historyRef = useRef<UndoEntry[]>([]);
  const nextId = useRef(1);
  const [toast, setToast] = useState<ToastState | null>(null);
  const toastRef = useRef<ToastState | null>(null);

  const commit = useCallback((next: UndoEntry[]) => {
    historyRef.current = next;
    setHistory(next);
  }, []);

  const show = useCallback((text: string, kind: ToastState["kind"], options: ShowOptions = {}) => {
    const next = { key: (toastRef.current?.key ?? 0) + 1, text, kind, ...options };
    toastRef.current = next;
    setToast(next);
  }, []);
  const hide = useCallback(() => {
    toastRef.current = null;
    setToast(null);
  }, []);

  const setUndone = useCallback(
    (entry: UndoEntry, undone: boolean) => {
      commit(historyRef.current.map((item) => (item.id === entry.id ? { ...item, undone } : item)));
    },
    [commit],
  );

  const undoEntry = useCallback(
    (entry: UndoEntry) => {
      entry.undo();
      setUndone(entry, true);
      show(t("toast.undone", { label: entry.label }), "done");
    },
    [setUndone, show, t],
  );

  const push = useCallback(
    (action: UndoableAction) => {
      const entry: UndoEntry = { ...action, id: nextId.current, undone: false };
      nextId.current += 1;
      commit([entry, ...historyRef.current]);
      show(action.label, "undoable", { ms: action.durationMs });
      return entry.id;
    },
    [commit, show],
  );

  const api = useMemo<UndoApi>(
    () => ({
      history,
      push,
      retract: (id) => {
        commit(historyRef.current.filter((entry) => entry.id !== id));
      },
      notify: (text) => {
        show(text, "done");
      },
      pending: (action) => {
        let settled = false;
        show(action.label, "pending", {
          cancel:
            action.cancel === undefined
              ? undefined
              : () => {
                  settled = true;
                  action.cancel?.();
                  show(action.cancelled ?? t("toast.cancelled", { label: action.label }), "done");
                },
        });
        // A later toast may have replaced this one; the outcome still shows.
        const settle = () => {
          const first = !settled;
          settled = true;
          return first;
        };
        return {
          done: (result) => {
            if (!settle()) {
              return;
            }
            if ("undo" in result) {
              push(result);
            } else {
              show(result.label, "done");
            }
          },
          fail: (failure) => {
            if (settle()) {
              show(failure.label, "failed", { retry: failure.retry });
            }
          },
        };
      },
      fail: (failure) => {
        show(failure.label, "failed", { retry: failure.retry });
      },
      undo: () => {
        const current = toastRef.current;
        if (current?.kind === "pending" && current.cancel !== undefined) {
          current.cancel();
          return;
        }
        const latest = historyRef.current.find((entry) => !entry.undone);
        if (latest !== undefined) {
          undoEntry(latest);
        }
      },
      toggle: (id) => {
        const entry = historyRef.current.find((item) => item.id === id);
        if (entry === undefined) {
          return;
        }
        if (!entry.undone) {
          undoEntry(entry);
        } else if (entry.redo !== undefined) {
          entry.redo();
          setUndone(entry, false);
          show(t("toast.redone", { label: entry.label }), "done");
        }
      },
    }),
    [history, push, commit, show, undoEntry, setUndone, t],
  );

  useShortcut("undo", () => {
    api.undo();
  });

  // Pending and failed toasts stay until they resolve or are dismissed.
  useEffect(() => {
    if (toast === null || toast.kind === "pending" || toast.kind === "failed") {
      return;
    }
    const timer = setTimeout(() => {
      if (toastRef.current === toast) {
        hide();
      }
    }, toast.ms ?? toastMs);
    return () => {
      clearTimeout(timer);
    };
  }, [toast, toastMs, hide]);

  return (
    <UndoContext value={api}>
      {children}
      <ToastRegion>
        {toast === null ? null : (
          <Toast
            key={toast.key}
            tone={
              toast.kind === "pending" ? "pending" : toast.kind === "failed" ? "alert" : "default"
            }
            onDismiss={toast.kind === "failed" ? hide : undefined}
            action={
              toast.kind === "undoable" ||
              (toast.kind === "pending" && toast.cancel !== undefined) ? (
                <Button variant="primary" shortcut="undo" onClick={api.undo}>
                  {t("undo")}
                </Button>
              ) : toast.kind === "failed" && toast.retry !== undefined ? (
                <Button
                  variant="primary"
                  onClick={() => {
                    hide();
                    toast.retry?.();
                  }}
                >
                  {t("toast.retry")}
                </Button>
              ) : undefined
            }
          >
            {toast.text}
          </Toast>
        )}
      </ToastRegion>
    </UndoContext>
  );
}

/** The undo stack: `push` after every action so it can be taken back. */
export function useUndoToast(): UndoApi {
  const api = use(UndoContext);
  if (api === null) {
    throw new Error("useUndoToast() must be used inside <UndoProvider>");
  }
  return api;
}
