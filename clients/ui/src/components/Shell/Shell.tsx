import type { ComponentPropsWithRef, ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { ariaKeyShortcut, useShortcutDefinition, useShortcuts } from "../../shortcuts";

import { cx } from "../../internal/cx";
import type { Accent } from "../../tokens/tokens";
import { Avatar } from "../Avatar/Avatar";
import { Icon } from "../Icon/Icon";
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

/**
 * The sidebar's "Search or ask" field: opens the command menu (⌘K, or / in a
 * workspace). It hints ⌘K only, so the label has room in every language; /
 * is in `aria-keyshortcuts` and the shortcut sheet.
 */
export function SearchButton({ onClick }: { onClick: () => void }) {
  const { t } = useTranslation("ui");
  const { platform } = useShortcuts();
  const definition = useShortcutDefinition("command.open");
  const slash = useShortcutDefinition("search.open");
  return (
    <button
      type="button"
      className={styles["search"]}
      onClick={onClick}
      aria-keyshortcuts={
        [definition, slash]
          .filter((each) => each !== undefined)
          .map((each) => ariaKeyShortcut(each.keys, platform))
          .join(" ") || undefined
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

/**
 * Below the large breakpoint a list and its reader are separate screens:
 * `hidden` takes this pane out there (the list while a conversation is
 * open, the reader while none is).
 */
type Narrow = "shown" | "hidden";

/** A list column (conversations of a stream). */
export function ListPane({
  label,
  narrow = "shown",
  children,
}: {
  label: string;
  narrow?: Narrow | undefined;
  children: ReactNode;
}) {
  return (
    <section className={styles["listPane"]} aria-label={label} data-narrow={narrow}>
      {children}
    </section>
  );
}

/** The reading column; `main` when it is the page's main content. */
export function ReaderPane({
  label,
  narrow = "shown",
  children,
}: {
  label: string;
  narrow?: Narrow | undefined;
  children: ReactNode;
}) {
  return (
    <main className={styles["reader"]} aria-label={label} data-narrow={narrow}>
      {children}
    </main>
  );
}

/** The padded, measure-limited column inside a reader; `wide` spans the whole main column (a table). */
export function ReaderBody({ children, wide = false }: { children: ReactNode; wide?: boolean }) {
  return (
    <div className={styles["readerBody"]} data-wide={wide || undefined}>
      {children}
    </div>
  );
}

export interface ReaderFooterProps {
  /** The reply composer while it is open; nothing (or `null`) otherwise. */
  slot?: ReactNode | undefined;
  /** From 48rem, shown while the slot is empty: the way to start a reply. */
  prompt?: ReactNode | undefined;
  /** Below 48rem, shown while the slot is empty: the floating action bar. */
  phoneBar?: ReactNode | undefined;
}

/**
 * The bottom of a reader, pinned while its thread scrolls under it, after
 * the {@link ReaderBody}: never between or after messages in the flow. An
 * open composer takes it and grows upward, scrolling past
 * `--reader-footer-max`; otherwise it shows `prompt` (from 48rem) or
 * `phoneBar` (below, above the tab bar).
 */
export function ReaderFooter({ slot, prompt, phoneBar }: ReaderFooterProps) {
  return (
    <div className={styles["readerFooter"]}>
      <div className={styles["readerFooterColumn"]}>
        <div className={styles["readerSlot"]}>{slot}</div>
        {prompt === undefined ? null : <div className={styles["readerPrompt"]}>{prompt}</div>}
        {phoneBar === undefined ? null : <div className={styles["readerPhoneBar"]}>{phoneBar}</div>}
      </div>
    </div>
  );
}

/** A pane's top bar: a title or actions on one line. */
export function PaneHeader({ children }: { children: ReactNode }) {
  return <div className={styles["paneHeader"]}>{children}</div>;
}

/** Fills the free space in a row (pushes what follows to the end). */
export function Spacer() {
  return <span className={styles["spacer"]} />;
}

/** The phone's top bar (below 48rem only): the workspace and the account. */
export function PhoneHeader({ children }: { children: ReactNode }) {
  return <header className={styles["phoneHeader"]}>{children}</header>;
}

/**
 * A second phone bar (below 48rem only), under the header: a page's own
 * sections as chips (Settings: Members, Mailboxes, Ways to sign in).
 */
export function PhoneSubnav({ label, children }: { label: string; children: ReactNode }) {
  return (
    <nav className={styles["phoneSubnav"]} aria-label={label}>
      {children}
    </nav>
  );
}

export interface AccountButtonProps extends Omit<ComponentPropsWithRef<"button">, "children"> {
  /** The signed-in person's name (falls back to the email). */
  name: string;
  email?: string | undefined;
  /** Just the avatar (phone header); the name becomes its label. */
  compact?: boolean | undefined;
  /** Accessible name of the compact button ("Account"). */
  label?: string | undefined;
}

/** Who is signed in, as the button that opens the account menu. */
export function AccountButton({
  name,
  email,
  compact = false,
  label,
  className,
  type = "button",
  ...rest
}: AccountButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles["account"], className)}
      data-compact={compact || undefined}
      aria-label={compact ? (label ?? name) : undefined}
    >
      <Avatar name={name} size={compact ? "md" : "sm"} />
      {compact ? null : (
        <>
          <span className={styles["accountText"]}>
            <span className={styles["accountName"]}>{name}</span>
            {email === undefined || email === name ? null : (
              <span className={styles["accountEmail"]}>{email}</span>
            )}
          </span>
          <Icon name="chevron-up-down" />
        </>
      )}
    </button>
  );
}

/** A workspace's colour mark with its initials, in that workspace's accent. */
export function WorkspaceMark({
  mark,
  accent,
  size = "md",
}: {
  mark: string;
  /** The workspace's accent preset; defaults to the accent in effect. */
  accent?: Accent | undefined;
  size?: "md" | "lg" | undefined;
}) {
  return (
    <span className={styles["mark"]} data-accent={accent} data-size={size} aria-hidden="true">
      {mark}
    </span>
  );
}
