import { useEffect, useRef, type CSSProperties } from "react";

import { Icon, type IconName } from "../Icon/Icon";
import { Kbd } from "../Kbd/Kbd";

import styles from "./SuggestionMenu.module.css";

export interface SuggestionItem {
  id: string;
  label: string;
  /** A line under the label (a person's address, a thread's sender). */
  detail?: string | undefined;
  /** A line icon, or a short glyph ("H1", "@"). */
  icon?: IconName | undefined;
  glyph?: string | undefined;
  /** The Markdown shortcut that makes the same block ("#", "[]"). */
  hint?: string | undefined;
}

export interface SuggestionGroup {
  id: string;
  /** The group's name ("Basic", "From Campbooks"). */
  label: string;
  items: readonly SuggestionItem[];
}

export interface SuggestionMenuProps {
  /** Accessible name ("Insert a block"). */
  label: string;
  groups: readonly SuggestionGroup[];
  /** The item the arrow keys are on. */
  active: string | undefined;
  onPick: (id: string) => void;
  /** Where it opens: the caret's corner, in viewport coordinates. */
  anchor: { x: number; y: number };
  /** Said when nothing matches. */
  empty: string;
  /** Said while the search is still running. */
  loading?: string | undefined;
}

/**
 * The menu under the caret that `/` and `@` open: groups of items, one
 * active, picked with Enter or a click. It is a listbox the editor drives
 * (focus stays in the text); the active item is announced through
 * `aria-activedescendant` on the editor by the caller's `activeId`.
 */
export function SuggestionMenu({
  label,
  groups,
  active,
  onPick,
  anchor,
  empty,
  loading,
}: SuggestionMenuProps) {
  const activeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    // jsdom has no scrollIntoView.
    const node: Partial<HTMLElement> | null = activeRef.current;
    node?.scrollIntoView?.({ block: "nearest" });
  }, [active]);
  const any = groups.some((group) => group.items.length > 0);
  return (
    <div
      className={styles["menu"]}
      role="listbox"
      aria-label={label}
      style={{ "--x": `${String(anchor.x)}px`, "--y": `${String(anchor.y)}px` } as CSSProperties}
    >
      {groups.map((group) =>
        group.items.length === 0 ? null : (
          <div key={group.id} className={styles["group"]}>
            <div className={styles["groupName"]}>{group.label}</div>
            {group.items.map((item) => (
              <button
                key={item.id}
                ref={item.id === active ? activeRef : undefined}
                id={`suggestion-${item.id}`}
                type="button"
                role="option"
                className={styles["item"]}
                aria-selected={item.id === active}
                onMouseDown={(event) => {
                  // Keep the caret in the text.
                  event.preventDefault();
                }}
                onClick={() => {
                  onPick(item.id);
                }}
              >
                <span className={styles["glyph"]} aria-hidden="true">
                  {item.icon === undefined ? item.glyph : <Icon name={item.icon} />}
                </span>
                <span className={styles["text"]}>
                  <span className={styles["label"]}>{item.label}</span>
                  {item.detail === undefined ? null : (
                    <span className={styles["detail"]}>{item.detail}</span>
                  )}
                </span>
                {item.hint === undefined ? null : <Kbd>{item.hint}</Kbd>}
              </button>
            ))}
          </div>
        ),
      )}
      {any ? null : <div className={styles["empty"]}>{loading ?? empty}</div>}
    </div>
  );
}
