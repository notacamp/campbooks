import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ariaKeyShortcut, useShortcutDefinition, useShortcuts } from "~/lib/shortcuts";

import { KbdHint } from "../Kbd/Kbd";
import { Visor, type VisorState } from "../Visor/Visor";

import styles from "./Shell.module.css";

/** The app frame: sidebar and panes side by side, wrapping on narrow screens. */
export function Shell({ children }: { children: ReactNode }) {
  return <div className={styles["shell"]}>{children}</div>;
}

/** The main navigation column. Hovering it reveals every nav shortcut. */
export function Sidebar({ children, label }: { children: ReactNode; label?: string }) {
  const { t } = useTranslation("ui");
  return (
    <nav className={styles["sidebar"]} aria-label={label ?? t("navigation.main")}>
      {children}
    </nav>
  );
}

/** Pinned to the bottom of the sidebar (assistant status, shortcuts button). */
export function SidebarFooter({ children }: { children: ReactNode }) {
  return <div className={styles["footer"]}>{children}</div>;
}

export interface WorkspaceSwitcherProps {
  /** Two letters on the accent-coloured mark. */
  mark: string;
  name: string;
  /** "Business", "Personal". */
  kind: string;
  onClick: () => void;
  /** Just the mark (phone header). */
  compact?: boolean | undefined;
}

export function WorkspaceSwitcher({
  mark,
  name,
  kind,
  onClick,
  compact = false,
}: WorkspaceSwitcherProps) {
  const { t } = useTranslation("ui");
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition("workspace.switch");
  return (
    <button
      type="button"
      className={styles["workspace"]}
      data-compact={compact || undefined}
      onClick={onClick}
      aria-label={t("workspace.current", { name })}
      aria-keyshortcuts={
        definition === undefined ? undefined : ariaKeyShortcut(definition.keys, platform)
      }
    >
      <span className={styles["mark"]} aria-hidden="true">
        {mark}
      </span>
      {compact ? null : (
        <>
          <span className={styles["workspaceText"]}>
            <span className={styles["workspaceName"]}>{name}</span>
            <span className={styles["workspaceKind"]}>{kind}</span>
          </span>
          <KbdHint shortcut="workspace.switch" />
        </>
      )}
    </button>
  );
}

/** The sidebar's "Search or ask" field: opens the command menu (⌘K). */
export function SearchButton({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation("ui");
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition("command.open");
  return (
    <button
      type="button"
      className={styles["search"]}
      onClick={onClick}
      aria-keyshortcuts={
        definition === undefined ? undefined : ariaKeyShortcut(definition.keys, platform)
      }
    >
      <Visor />
      <span className={styles["searchLabel"]}>{t("search.label")}</span>
      <KbdHint shortcut="command.open" />
    </button>
  );
}

export interface StatusLineProps {
  visor?: VisorState | undefined;
  /** Bold first line: "Reading 3 new messages". */
  title: string;
  children?: ReactNode | undefined;
}

/** What the assistant is doing right now, next to its visor. */
export function StatusLine({ visor = "idle", title, children }: StatusLineProps) {
  return (
    <div className={styles["status"]}>
      <Visor state={visor} size="md" />
      <span>
        <b className={styles["statusTitle"]}>{title}</b>
        {children}
      </span>
    </div>
  );
}

/** A list column (conversations of a stream). */
export function ListPane({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className={styles["listPane"]} aria-label={label}>
      {children}
    </section>
  );
}

/** The reading column; `main` when it is the page's main content. */
export function ReaderPane({ label, children }: { label: string; children: ReactNode }) {
  return (
    <main className={styles["reader"]} aria-label={label}>
      {children}
    </main>
  );
}

/** The padded, measure-limited column inside a reader. */
export function ReaderBody({ children }: { children: ReactNode }) {
  return <div className={styles["readerBody"]}>{children}</div>;
}

/** A pane's top bar: a title or actions on one line. */
export function PaneHeader({ children }: { children: ReactNode }) {
  return <div className={styles["paneHeader"]}>{children}</div>;
}

/** Fills the free space in a row (pushes what follows to the end). */
export function Spacer() {
  return <span className={styles["spacer"]} />;
}
