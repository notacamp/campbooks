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
import { Visor } from "../Visor/Visor";

import styles from "./Toast.module.css";

export interface ToastProps {
  children: ReactNode;
  /** One action, usually Undo. */
  action?: ReactNode | undefined;
  /** `alert` for failures that need the user (a send failed): wraps, and is announced assertively. */
  tone?: "default" | "alert" | undefined;
}

/** A short confirmation pill. Put it in a {@link ToastRegion} so it is announced. */
export function Toast({ children, action, tone = "default" }: ToastProps) {
  return (
    <div className={styles["toast"]} data-tone={tone} role={tone === "alert" ? "alert" : undefined}>
      <Visor state={tone === "alert" ? "alert" : "happy"} />
      <span className={styles["message"]}>{children}</span>
      {action}
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
}

export interface UndoEntry extends UndoableAction {
  id: number;
  undone: boolean;
}

export interface UndoApi {
  /** Records an action that just happened and shows it with Undo. */
  push: (action: UndoableAction) => void;
  /** Undoes the most recent action that is not undone yet (Z). */
  undo: () => void;
  /** Undo or redo one entry of the history. */
  toggle: (id: number) => void;
  /** Newest first, for the session. */
  history: readonly UndoEntry[];
}

const UndoContext = createContext<UndoApi | null>(null);

interface ToastState {
  key: number;
  text: string;
  undoable: boolean;
}

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

  const commit = useCallback((next: UndoEntry[]) => {
    historyRef.current = next;
    setHistory(next);
  }, []);

  const show = useCallback((text: string, undoable: boolean) => {
    setToast((current) => ({ key: (current?.key ?? 0) + 1, text, undoable }));
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
      show(t("toast.undone", { label: entry.label }), false);
    },
    [setUndone, show, t],
  );

  const api = useMemo<UndoApi>(
    () => ({
      history,
      push: (action) => {
        const entry: UndoEntry = { ...action, id: nextId.current, undone: false };
        nextId.current += 1;
        commit([entry, ...historyRef.current]);
        show(action.label, true);
      },
      undo: () => {
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
          show(t("toast.redone", { label: entry.label }), false);
        }
      },
    }),
    [history, commit, show, undoEntry, setUndone, t],
  );

  useShortcut("undo", () => {
    api.undo();
  });

  useEffect(() => {
    if (toast === null) {
      return;
    }
    const timer = setTimeout(() => {
      setToast(null);
    }, toastMs);
    return () => {
      clearTimeout(timer);
    };
  }, [toast, toastMs]);

  return (
    <UndoContext value={api}>
      {children}
      <ToastRegion>
        {toast === null ? null : (
          <Toast
            key={toast.key}
            action={
              toast.undoable ? (
                <Button variant="primary" shortcut="undo" onClick={api.undo}>
                  {t("undo")}
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
