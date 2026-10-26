import { useRef, type ComponentPropsWithRef, type KeyboardEvent, type ReactNode } from "react";

import { cx, Icon, IconButton, KbdHint, type IconName } from "@notacamp/ui";

import styles from "./Toolbar.module.css";

export interface TableTab {
  id: string;
  label: string;
  icon: IconName;
  current: boolean;
  onSelect: () => void;
  /** Beside the current tab (its menu). */
  after?: ReactNode | undefined;
}

export interface TableTabsProps {
  /** Names the tabs ("Views"). */
  label: string;
  tabs: readonly TableTab[];
  /** After the last tab (the "+" that adds a view). */
  trailing?: ReactNode | undefined;
  /** The id of the panel the current tab shows. */
  panelId?: string | undefined;
}

/**
 * The views of a table as tabs: the current one is underlined, arrows move
 * between them, and the row scrolls sideways when they do not all fit.
 */
export function TableTabs({ label, tabs, trailing, panelId }: TableTabsProps) {
  const list = useRef<HTMLDivElement>(null);
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") {
      return;
    }
    const buttons = [...(list.current?.querySelectorAll<HTMLElement>('[role="tab"]') ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLElement);
    if (at === -1 || buttons.length === 0) {
      return;
    }
    event.preventDefault();
    const next = (at + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
    buttons[next]?.click();
  };
  return (
    <div className={styles["tabs"]}>
      <div
        ref={list}
        role="tablist"
        tabIndex={-1}
        aria-label={label}
        className={styles["tablist"]}
        onKeyDown={onKeyDown}
      >
        {tabs.map((tab) => (
          <span key={tab.id} className={styles["tabWrap"]} data-current={tab.current || undefined}>
            <button
              type="button"
              role="tab"
              aria-selected={tab.current}
              aria-controls={tab.current ? panelId : undefined}
              tabIndex={tab.current ? 0 : -1}
              className={styles["tab"]}
              onClick={tab.onSelect}
            >
              <Icon name={tab.icon} />
              <span className={styles["tabName"]}>{tab.label}</span>
            </button>
            {tab.current ? tab.after : null}
          </span>
        ))}
      </div>
      {trailing}
    </div>
  );
}

export interface TableToolbarProps {
  /** The view tabs. */
  tabs: ReactNode;
  /** The filter chips (they give way first). */
  chips?: ReactNode | undefined;
  /** Filter, Sort, New. */
  actions: ReactNode;
}

/** One row: the tabs, the filter chips and the actions. It never wraps. */
export function TableToolbar({ tabs, chips, actions }: TableToolbarProps) {
  return (
    <div className={styles["toolbar"]}>
      {tabs}
      {chips === undefined ? null : <div className={styles["chips"]}>{chips}</div>}
      <div className={styles["actions"]}>{actions}</div>
    </div>
  );
}

export interface ToolbarButtonProps extends Omit<ComponentPropsWithRef<"button">, "children"> {
  icon: IconName;
  /** Its name: the text from the tablet width up, the accessible name always. */
  label: string;
  /** How many filters or sorts are on. */
  count?: number | undefined;
  primary?: boolean | undefined;
  /** The shortcut that does the same: its keys are shown after the name. */
  shortcut?: string | undefined;
}

/** A toolbar button: icon and name, the name only from the tablet width up. */
export function ToolbarButton({
  icon,
  label,
  count,
  primary = false,
  shortcut,
  className,
  type = "button",
  ...rest
}: ToolbarButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      aria-label={count === undefined || count === 0 ? label : `${label} (${String(count)})`}
      title={label}
      className={cx(styles["button"], className)}
      data-primary={primary || undefined}
    >
      <Icon name={icon} />
      <span className={styles["buttonLabel"]}>{label}</span>
      {count === undefined || count === 0 ? null : <span className={styles["count"]}>{count}</span>}
      {shortcut === undefined ? null : (
        <span className={styles["keys"]}>
          <KbdHint shortcut={shortcut} />
        </span>
      )}
    </button>
  );
}

export interface FilterChipProps {
  children: ReactNode;
  removeLabel: string;
  onRemove: () => void;
  /** Open the filter builder on this condition. */
  onOpen?: (() => void) | undefined;
}

/** A condition of the filter, as a chip with an x. */
export function FilterChip({ children, removeLabel, onRemove, onOpen }: FilterChipProps) {
  return (
    <span className={styles["chip"]}>
      {onOpen === undefined ? (
        <span className={styles["chipText"]}>{children}</span>
      ) : (
        <button type="button" className={styles["chipText"]} onClick={onOpen}>
          {children}
        </button>
      )}
      <IconButton icon="close" label={removeLabel} onClick={onRemove} />
    </span>
  );
}
