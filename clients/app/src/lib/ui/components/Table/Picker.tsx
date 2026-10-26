import { useId, useState, type KeyboardEvent, type ReactNode } from "react";

import { Icon } from "@notacamp/ui";

import styles from "./Picker.module.css";

export interface PickerItem {
  id: string;
  /** What is drawn: a tag, a name with its avatar... */
  label: ReactNode;
  /** The item's name for screen readers. */
  text: string;
  selected?: boolean | undefined;
  /** A quiet line after it. */
  hint?: string | undefined;
  /** Something at the end (an edit button). It is outside the option's own press. */
  end?: ReactNode | undefined;
}

export interface PickerProps {
  /** Names the list ("Stage"). */
  label: string;
  /** Names the search field ("Search options"). */
  searchLabel: string;
  placeholder?: string | undefined;
  query: string;
  onQuery: (query: string) => void;
  items: readonly PickerItem[];
  onPick: (id: string) => void;
  /** Several can be on at once. */
  multiple?: boolean | undefined;
  /** Said when nothing matches. */
  empty: string;
  /** "Create Foo": offered while something is typed that matches nothing. */
  create?: { label: string; onCreate: () => void } | undefined;
  /** Said while the list is loading. */
  loading?: string | undefined;
  /** After the list (a Clear button, "Edit options"). */
  footer?: ReactNode | undefined;
}

/**
 * A search box over a list to pick from (an ARIA combobox): the focus stays
 * in the box, arrows move the highlight, Enter picks it. It is the one
 * picker of options, people, rows and Campbooks records: always by name.
 */
export function Picker({
  label,
  searchLabel,
  placeholder,
  query,
  onQuery,
  items,
  onPick,
  multiple = false,
  empty,
  create,
  loading,
  footer,
}: PickerProps) {
  const listId = useId();
  const [highlight, setHighlight] = useState(0);
  const total = items.length + (create === undefined ? 0 : 1);
  const at = Math.min(highlight, Math.max(0, total - 1));
  const optionId = (index: number) => `${listId}-${String(index)}`;

  const choose = (index: number) => {
    const item = items[index];
    if (item === undefined) {
      create?.onCreate();
    } else {
      onPick(item.id);
    }
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlight(total === 0 ? 0 : (at + 1) % total);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlight(total === 0 ? 0 : (at - 1 + total) % total);
    } else if (event.key === "Enter" && total > 0) {
      event.preventDefault();
      choose(at);
    }
  };

  return (
    <div className={styles["picker"]}>
      <input
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls={listId}
        aria-activedescendant={total === 0 ? undefined : optionId(at)}
        aria-autocomplete="list"
        aria-label={searchLabel}
        placeholder={placeholder}
        className={styles["search"]}
        value={query}
        autoComplete="off"
        onChange={(event) => {
          setHighlight(0);
          onQuery(event.target.value);
        }}
        onKeyDown={onKeyDown}
      />
      <div
        id={listId}
        role="listbox"
        aria-label={label}
        aria-multiselectable={multiple || undefined}
        className={styles["list"]}
      >
        {items.map((item, index) => (
          <div key={item.id} role="presentation" className={styles["entry"]}>
            <div
              id={optionId(index)}
              role="option"
              tabIndex={-1}
              aria-selected={item.selected === true}
              aria-label={item.text}
              className={styles["option"]}
              data-highlight={at === index || undefined}
              onMouseDown={(event) => {
                // The focus stays in the search field.
                event.preventDefault();
              }}
              onClick={() => {
                onPick(item.id);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onPick(item.id);
                }
              }}
            >
              <span className={styles["check"]}>
                {item.selected === true ? <Icon name="check" /> : null}
              </span>
              <span className={styles["label"]}>{item.label}</span>
              {item.hint === undefined ? null : <span className={styles["hint"]}>{item.hint}</span>}
            </div>
            {item.end}
          </div>
        ))}
        {create === undefined ? null : (
          <div role="presentation" className={styles["entry"]}>
            <div
              id={optionId(items.length)}
              role="option"
              tabIndex={-1}
              aria-selected="false"
              className={styles["option"]}
              data-highlight={at === items.length || undefined}
              onMouseDown={(event) => {
                event.preventDefault();
              }}
              onClick={create.onCreate}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  create.onCreate();
                }
              }}
            >
              <span className={styles["check"]}>
                <Icon name="plus" />
              </span>
              <span className={styles["label"]}>{create.label}</span>
            </div>
          </div>
        )}
      </div>
      {total === 0 ? (
        <p className={styles["empty"]} role="status">
          {loading ?? empty}
        </p>
      ) : null}
      {footer === undefined ? null : <div className={styles["footer"]}>{footer}</div>}
    </div>
  );
}
