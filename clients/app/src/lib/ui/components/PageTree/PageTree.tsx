import type { CSSProperties, DragEventHandler, ReactNode } from "react";

import { Icon, type IconName, IconButton } from "@notacamp/ui";

import styles from "./PageTree.module.css";

export interface PageTreeProps {
  /** Accessible name of the tree ("Pages"). */
  label: string;
  children: ReactNode;
}

/** The page tree: groups of rows, one level of indent per depth. */
export function PageTree({ label, children }: PageTreeProps) {
  return (
    <nav className={styles["tree"]} aria-label={label}>
      {children}
    </nav>
  );
}

export interface PageTreeGroupProps {
  /** The group's name ("Workspace", "Private", "Shared with you"). */
  name: string;
  /** A control at the end of the heading (a "New page" button). */
  action?: ReactNode | undefined;
  /** Said when the group has no rows. */
  empty?: string | undefined;
  children?: ReactNode | undefined;
}

/** A named group of rows. */
export function PageTreeGroup({ name, action, empty, children }: PageTreeGroupProps) {
  const hasRows = children !== undefined && children !== null && children !== false;
  return (
    <section className={styles["group"]}>
      <div className={styles["heading"]}>
        <h2 className={styles["name"]}>{name}</h2>
        {action}
      </div>
      {hasRows ? <div className={styles["rows"]}>{children}</div> : null}
      {!hasRows && empty !== undefined ? <p className={styles["empty"]}>{empty}</p> : null}
    </section>
  );
}

export interface PageTreeRowProps {
  /** The page's title, as shown. */
  label: string;
  /** Its icon. */
  icon?: IconName | undefined;
  /** How deep it is; 0 is the top. */
  depth?: number | undefined;
  /** Open (true), closed (false), or no sub-pages (undefined). */
  expanded?: boolean | undefined;
  /** The page on screen. */
  current?: boolean | undefined;
  /** Open the page. */
  onOpen: () => void;
  /** Show or hide the sub-pages. */
  onToggle?: (() => void) | undefined;
  /** The toggle's name ("Show sub-pages of Team handbook"). */
  toggleLabel?: string | undefined;
  /** Add a sub-page (on hover and focus). */
  onAdd?: (() => void) | undefined;
  /** The add button's name ("Add a sub-page to Team handbook"). */
  addLabel?: string | undefined;
  /** More actions for the row (a menu), after the add button. */
  menu?: ReactNode | undefined;
  /** The row can be dragged to move the page. */
  draggable?: boolean | undefined;
  /** Where a dragged page would land: before this row, inside it or after it. */
  dropZone?: "before" | "inside" | "after" | undefined;
  /** This row is being dragged. */
  dragging?: boolean | undefined;
  onDragStart?: DragEventHandler<HTMLDivElement> | undefined;
  onDragOver?: DragEventHandler<HTMLDivElement> | undefined;
  onDragLeave?: DragEventHandler<HTMLDivElement> | undefined;
  onDrop?: DragEventHandler<HTMLDivElement> | undefined;
  onDragEnd?: DragEventHandler<HTMLDivElement> | undefined;
}

/**
 * A page in the tree: a toggle for its sub-pages, its icon and title as one
 * button, and on hover the add button and a menu. The toggle, the title and
 * each action are separate buttons.
 */
export function PageTreeRow({
  label,
  icon = "document",
  depth = 0,
  expanded,
  current = false,
  onOpen,
  onToggle,
  toggleLabel,
  onAdd,
  addLabel,
  menu,
  draggable = false,
  dropZone,
  dragging = false,
  onDragStart,
  onDragOver,
  onDragLeave,
  onDrop,
  onDragEnd,
}: PageTreeRowProps) {
  return (
    <div
      role="presentation"
      className={styles["row"]}
      style={{ "--depth": depth } as CSSProperties}
      data-current={current || undefined}
      data-drop={dropZone}
      data-dragging={dragging || undefined}
      draggable={draggable || undefined}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
    >
      {expanded === undefined ? (
        <span className={styles["spacer"]} aria-hidden="true" />
      ) : (
        <button
          type="button"
          className={styles["toggle"]}
          aria-expanded={expanded}
          aria-label={toggleLabel}
          onClick={onToggle}
        >
          <Icon name={expanded ? "chevron-down" : "chevron-right"} />
        </button>
      )}
      <button
        type="button"
        className={styles["open"]}
        aria-current={current ? "page" : undefined}
        onClick={onOpen}
      >
        <Icon name={icon} />
        <span className={styles["title"]}>{label}</span>
      </button>
      <span className={styles["actions"]}>
        {onAdd === undefined ? null : (
          <IconButton icon="plus" label={addLabel ?? label} onClick={onAdd} />
        )}
        {menu}
      </span>
    </div>
  );
}
