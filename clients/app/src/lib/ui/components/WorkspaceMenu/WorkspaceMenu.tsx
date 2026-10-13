import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import type { ShortcutId } from "~/lib/shortcuts";

import { Overlay } from "../../internal/Overlay";
import type { Accent } from "../../tokens/tokens";
import { Icon } from "../Icon/Icon";
import { Kbd, KbdHint } from "../Kbd/Kbd";
import { WorkspaceMark } from "../Shell/Shell";

import styles from "./WorkspaceMenu.module.css";

export interface WorkspaceMenuItem {
  id: string;
  name: string;
  /** Two letters on the mark. */
  mark: string;
  /** "Business", "Personal" (translated by the caller). */
  kind: string;
  accent: Accent;
}

export interface WorkspaceMenuProps {
  open: boolean;
  onClose: () => void;
  workspaces: readonly WorkspaceMenuItem[];
  currentId?: string | undefined;
  onSelect: (id: string) => void;
  /** Offer "Create a workspace" at the end. */
  onCreate?: (() => void) | undefined;
  /** The shortcut that creates a workspace, hinted on that row. */
  createShortcut?: ShortcutId | undefined;
}

/**
 * ⌘⇧W: every workspace the user belongs to, each in its own accent, the
 * current one marked. ↑ ↓ move, ↵ opens.
 */
export function WorkspaceMenu({
  open,
  onClose,
  workspaces,
  currentId,
  onSelect,
  onCreate,
  createShortcut,
}: WorkspaceMenuProps) {
  const { t } = useTranslation("ui");
  const list = useRef<HTMLDivElement>(null);

  // ↑ ↓ move between rows (Tab works too); delegated so rows stay plain buttons.
  useEffect(() => {
    const element = list.current;
    if (!open || element === null) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
        return;
      }
      const buttons = [...element.querySelectorAll<HTMLButtonElement>("button")];
      const focused = document.activeElement;
      const index = focused instanceof HTMLButtonElement ? buttons.indexOf(focused) : -1;
      const step = event.key === "ArrowDown" ? 1 : -1;
      const next = buttons[(index + step + buttons.length) % buttons.length];
      if (next !== undefined) {
        event.preventDefault();
        next.focus();
      }
    };
    element.addEventListener("keydown", onKeyDown);
    return () => {
      element.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <Overlay open={open} onClose={onClose} label={t("workspace.switch")}>
      <div className={styles["head"]}>
        <h2 className={styles["title"]}>{t("workspace.switch")}</h2>
        <Kbd>Esc</Kbd>
      </div>
      <div ref={list} className={styles["list"]}>
        {workspaces.map((workspace) => {
          const current = workspace.id === currentId;
          return (
            <button
              key={workspace.id}
              type="button"
              className={styles["row"]}
              aria-current={current ? "true" : undefined}
              data-autofocus={current || undefined}
              onClick={() => {
                onClose();
                onSelect(workspace.id);
              }}
            >
              <WorkspaceMark mark={workspace.mark} accent={workspace.accent} />
              <span className={styles["text"]}>
                <span className={styles["name"]}>{workspace.name}</span>
                <span className={styles["kind"]}>{workspace.kind}</span>
              </span>
              {current ? <Icon name="check" /> : null}
            </button>
          );
        })}
        {onCreate === undefined ? null : (
          <button
            type="button"
            className={styles["row"]}
            data-create=""
            onClick={() => {
              onClose();
              onCreate();
            }}
          >
            <span className={styles["plus"]} aria-hidden="true">
              <Icon name="plus" />
            </span>
            <span className={styles["text"]}>
              <span className={styles["name"]}>{t("workspace.create")}</span>
            </span>
            {createShortcut === undefined ? null : <KbdHint shortcut={createShortcut} />}
          </button>
        )}
      </div>
    </Overlay>
  );
}
