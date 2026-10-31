import type { ComponentPropsWithRef, ReactNode } from "react";

import { Icon, IconButton, type IconName } from "@notacamp/ui";

import styles from "./Cells.module.css";

/** The colours an option may have, as the server names them. */
export type OptionTagColor =
  "default" | "gray" | "brown" | "orange" | "yellow" | "green" | "blue" | "purple" | "pink" | "red";

export interface OptionTagProps {
  color: OptionTagColor;
  children: ReactNode;
  /** Take it off (the "x" of a chosen option); its name for screen readers. */
  onRemove?: (() => void) | undefined;
  removeLabel?: string | undefined;
}

/** An option of a select, as a tinted label. */
export function OptionTag({ color, children, onRemove, removeLabel }: OptionTagProps) {
  return (
    <span className={styles["option"]} data-color={color}>
      <span
        className={styles["optionText"]}
        title={typeof children === "string" ? children : undefined}
      >
        {children}
      </span>
      {onRemove === undefined ? null : (
        <IconButton icon="close" label={removeLabel ?? ""} size="md" onClick={onRemove} />
      )}
    </span>
  );
}

export interface TypeGlyphProps {
  /** A short mark for the type ("Aa", "#", "@"). */
  glyph: string;
}

/** The small boxed mark that says what type a column is. */
export function TypeGlyph({ glyph }: TypeGlyphProps) {
  return (
    <span className={styles["glyph"]} aria-hidden="true">
      {glyph}
    </span>
  );
}

export interface CellLineProps {
  children: ReactNode;
  /** Right-aligned (numbers). */
  end?: boolean | undefined;
  /** The whole value, for when the cell cuts it short (defaults to the content when it is text). */
  title?: string | undefined;
}

/** What a cell shows: its content on one line, ending in an ellipsis at the cell's edge, the whole value in its title. */
export function CellLine({ children, end = false, title }: CellLineProps) {
  const whole = title ?? (typeof children === "string" ? children : undefined);
  return (
    <span className={styles["line"]} data-end={end || undefined} title={whole}>
      {children}
    </span>
  );
}

/** Several things side by side in a cell (tags, chips): they scroll nowhere, they are cut at the edge. */
export function CellRow({ children }: { children: ReactNode }) {
  return <span className={styles["row"]}>{children}</span>;
}

export interface CellLinkProps {
  href: string;
  children: ReactNode;
}

/** A web or mail address in a cell. Only the caller's safe addresses reach it. */
export function CellLink({ href, children }: CellLinkProps) {
  return (
    <a
      className={styles["link"]}
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      title={typeof children === "string" ? children : undefined}
    >
      {children}
      <Icon name="arrow-right" />
    </a>
  );
}

/** The field a text, number or address cell turns into while it is edited. */
export function CellInput(props: Omit<ComponentPropsWithRef<"input">, "className">) {
  return <input {...props} className={styles["input"]} />;
}

/** A cell's content as the button that opens its editor (a picker in a popover). */
export function CellButton({ children, ...rest }: ComponentPropsWithRef<"button">) {
  return (
    <button type="button" {...rest} className={styles["button"]}>
      {children}
    </button>
  );
}

export interface HeaderButtonProps extends Omit<ComponentPropsWithRef<"button">, "children"> {
  glyph: string;
  /** The column's name. */
  children: ReactNode;
  /** A mark after the name (the sort arrow). */
  trailing?: ReactNode | undefined;
}

/** A column's name in the header: its type's mark, then the name; it opens the column's menu. */
export function HeaderButton({
  glyph,
  children,
  trailing,
  type = "button",
  ...rest
}: HeaderButtonProps) {
  return (
    <button {...rest} type={type} className={styles["headerButton"]}>
      <TypeGlyph glyph={glyph} />
      <span className={styles["headerName"]}>{children}</span>
      {trailing}
    </button>
  );
}

export interface ColumnMarkProps {
  icon: IconName;
  /** Why the mark is there ("Read-only: computed in Notion"): its accessible name and tooltip. */
  label: string;
}

/** A small mark beside a column's name: a lock for what only Notion changes, a link for what stays here. */
export function ColumnMark({ icon, label }: ColumnMarkProps) {
  return (
    <span role="img" aria-label={label} title={label} className={styles["mark"]}>
      <Icon name={icon} />
    </span>
  );
}

/** What an empty value says in a place that invites a click ("Empty"). */
export function CellEmpty({ children }: { children: ReactNode }) {
  return <span className={styles["empty"]}>{children}</span>;
}

export interface SummaryButtonProps extends Omit<ComponentPropsWithRef<"button">, "children"> {
  /** What it does ("Summarise Value"). */
  label: string;
  /** The summary shown ("Sum €28,400"); none shows a quiet plus. */
  text?: string | undefined;
}

/** The footer's button under a column: its summary, or a quiet plus to choose one. */
export function SummaryButton({ label, text, type = "button", ...rest }: SummaryButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      className={styles["summary"]}
      aria-label={text === undefined ? label : `${label}: ${text}`}
      title={text === undefined ? label : `${label}: ${text}`}
    >
      {text ?? <Icon name="plus" />}
    </button>
  );
}
