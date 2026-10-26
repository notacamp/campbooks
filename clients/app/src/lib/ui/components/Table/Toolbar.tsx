import {
  useLayoutEffect,
  useRef,
  useState,
  type ComponentPropsWithRef,
  type KeyboardEvent,
  type ReactNode,
} from "react";

import { cx, Icon, IconButton, KbdHint, Popover, type IconName } from "@notacamp/ui";

import { MenuItem, MenuList } from "./Menu";

import styles from "./Toolbar.module.css";

export interface TableTab {
  id: string;
  label: string;
  icon: IconName;
  current: boolean;
  onSelect: () => void;
}

export interface TableToolbarProps {
  /** Names the tabs ("Views"). */
  tabsLabel: string;
  tabs: readonly TableTab[];
  /** The id of the panel the current tab shows. */
  panelId?: string | undefined;
  /** The "More views" button's name: the views that do not fit are in its menu. */
  moreViewsLabel: string;
  /** After the tabs, outside the tab list: the current view's options and the "+" that adds one. */
  trailing?: ReactNode | undefined;
  /** The filter chips: they go first when the row is tight (the Filter button still counts them). */
  chips?: ReactNode | undefined;
  /** Filter, Sort, New. */
  actions: ReactNode;
}

interface Fit {
  chips: boolean;
  /** The tabs in the row, or all of them when `null`. */
  shown: string | null;
}

const WIDE: Fit = { chips: true, shown: null };

/** The width of an element, as the layout has it. */
const widthOf = (element: Element | null | undefined) =>
  element instanceof HTMLElement ? element.offsetWidth : 0;

/**
 * One row, whatever the width of the box it is in: the tabs, the filter
 * chips and the actions. It never wraps and nothing overlaps. When it is
 * tight the chips go first (the Filter button keeps the count and opens the
 * builder), then the tabs that do not fit move into a "More views" menu; the
 * current view always stays. It measures its own box, not the screen.
 */
export function TableToolbar({
  tabsLabel,
  tabs,
  panelId,
  moreViewsLabel,
  trailing,
  chips,
  actions,
}: TableToolbarProps) {
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [fit, setFit] = useState<Fit>(WIDE);
  // What each part took when it was last drawn: a part that is folded keeps its last width.
  const sizes = useRef({ tabs: new Map<string, number>(), chips: 0, rest: 0, more: 0 });

  const hasChips = chips !== undefined;
  const key = tabs.map((tab) => `${tab.id}:${tab.label}:${String(tab.current)}`).join("|");

  useLayoutEffect(() => {
    const box = root.current;
    if (box === null) {
      return;
    }
    const measure = () => {
      const seen = sizes.current;
      for (const element of box.querySelectorAll<HTMLElement>("[data-tab]")) {
        // Its natural width: what a squeezed tab hides of its name counts.
        const name = element.querySelector<HTMLElement>("[data-name]");
        const hidden = name === null ? 0 : Math.max(0, name.scrollWidth - name.offsetWidth);
        seen.tabs.set(element.dataset["tab"] ?? "", element.offsetWidth + hidden);
      }
      const chipsBox = box.querySelector<HTMLElement>("[data-part='chips']");
      if (chipsBox !== null && chipsBox.offsetWidth > 0) {
        seen.chips = chipsBox.scrollWidth;
      }
      seen.rest = widthOf(box.querySelector("[data-part='rest']"));
      const more = box.querySelector("[data-part='more']");
      if (more !== null) {
        seen.more = widthOf(more);
      }
      const gap = Number.parseFloat(getComputedStyle(box).columnGap) || 0;
      const available = box.clientWidth;
      const tabWidths = tabs.map((tab) => seen.tabs.get(tab.id) ?? 0);
      const all = tabWidths.reduce((sum, width) => sum + width, 0);
      const parts = (chipsOn: boolean) => 3 + (chipsOn ? 1 : 0);
      const fits = (chipsOn: boolean) =>
        all + seen.rest + (chipsOn ? seen.chips : 0) + gap * (parts(chipsOn) - 1) <= available;
      let next: Fit;
      if (fits(true) || (!hasChips && fits(false))) {
        next = WIDE;
      } else if (fits(false)) {
        next = { chips: false, shown: null };
      } else {
        // Keep the current tab, then as many of the others as fit beside the "More views" button.
        const room = available - seen.rest - (seen.more === 0 ? 44 : seen.more) - gap * 3;
        let used = 0;
        const keep = new Set<string>();
        const current = tabs.find((tab) => tab.current);
        if (current !== undefined) {
          keep.add(current.id);
          used += seen.tabs.get(current.id) ?? 0;
        }
        for (const tab of tabs) {
          const width = seen.tabs.get(tab.id) ?? 0;
          if (!keep.has(tab.id) && used + width <= room) {
            keep.add(tab.id);
            used += width;
          }
        }
        next = {
          chips: false,
          shown: tabs
            .filter((tab) => keep.has(tab.id))
            .map((tab) => tab.id)
            .join("|"),
        };
      }
      setFit((current) =>
        current.chips === next.chips && current.shown === next.shown ? current : next,
      );
    };
    measure();
    if (typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(measure);
    observer.observe(box);
    return () => {
      observer.disconnect();
    };
    // The measure reads the latest tabs and chips; it runs again when they change.
  }, [key, hasChips, chips, trailing, actions, tabs]);

  const shown = fit.shown === null ? null : new Set(fit.shown.split("|"));
  const inRow = tabs.filter((tab) => shown === null || shown.has(tab.id));
  const folded = tabs.filter((tab) => shown !== null && !shown.has(tab.id));

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
    <div ref={root} className={styles["toolbar"]}>
      <div
        ref={list}
        role="tablist"
        tabIndex={-1}
        aria-label={tabsLabel}
        className={styles["tablist"]}
        onKeyDown={onKeyDown}
      >
        {inRow.map((tab) => (
          <span
            key={tab.id}
            role="presentation"
            data-tab={tab.id}
            className={styles["tabWrap"]}
            data-current={tab.current || undefined}
          >
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
              <span data-name className={styles["tabName"]}>
                {tab.label}
              </span>
            </button>
          </span>
        ))}
      </div>
      {folded.length === 0 ? null : (
        <span data-part="more" className={styles["more"]}>
          <Popover
            label={moreViewsLabel}
            placement="bottom-start"
            layer="top"
            renderTrigger={(trigger) => (
              <IconButton {...trigger} icon="more" label={moreViewsLabel} />
            )}
          >
            {(close) => (
              <MenuList label={moreViewsLabel}>
                {folded.map((tab) => (
                  <MenuItem
                    key={tab.id}
                    icon={tab.icon}
                    onSelect={() => {
                      close();
                      tab.onSelect();
                    }}
                  >
                    {tab.label}
                  </MenuItem>
                ))}
              </MenuList>
            )}
          </Popover>
        </span>
      )}
      <div data-part="rest" className={styles["rest"]}>
        {trailing}
      </div>
      <span className={styles["spacer"]} />
      {hasChips && fit.chips ? (
        <div data-part="chips" className={styles["chips"]}>
          {chips}
        </div>
      ) : null}
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
