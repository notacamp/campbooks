import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { useShortcutDefinitions, useShortcuts, type ShortcutId } from "~/lib/shortcuts";

import { Overlay } from "../../internal/Overlay";
import { useTranslateKey } from "../../internal/translate-key";
import { Kbd, KbdHint } from "../Kbd/Kbd";
import { SearchFieldChips, type SearchFieldChip } from "../SearchField/SearchField";
import { Text } from "../Typography/Typography";
import { Visor } from "../Visor/Visor";

import styles from "./CommandMenu.module.css";

export interface Command {
  id: string;
  label: string;
  /** Heading it is listed under ("Suggested", "Go to"). */
  group: string;
  /** Hint this shortcut's keys next to the command. */
  shortcut?: ShortcutId | undefined;
  onSelect: () => void;
}

/** One row of an extra section. Plain text by default; `render` draws it itself. */
export interface MenuOption {
  id: string;
  label: string;
  /** Runs on ↵ or a click, after the menu closes. */
  onSelect: () => void;
  /** Stay open after it runs (a suggestion that fills the field). */
  keepOpen?: boolean | undefined;
  /**
   * Draw the row: it must be an `option` with this `id`, `aria-selected` from
   * `selected`, and call `select` when clicked.
   */
  render?: ((row: { id: string; selected: boolean; select: () => void }) => ReactNode) | undefined;
}

/** What a screen adds above the commands, driven by what is typed. */
export interface MenuSection {
  /** Heading of its rows. */
  group: string;
  /** Rows, first in the one keyboard order (↑ ↓ ↵ run across every group). */
  options: MenuOption[];
  /** Under the heading, above the rows: notes, a loading state, a problem. */
  status?: ReactNode;
  /** Filters understood from the query, removable, under the input. */
  chips?: readonly SearchFieldChip[] | undefined;
  /** ⌘↵ with a query: close and show everything. */
  onSubmitAll?: (() => void) | undefined;
  /** Its rows are still on their way: ↵ shows everything rather than run a command. */
  pending?: boolean | undefined;
}

/** What a section hook is given: the query, and a way to rewrite it. */
export interface MenuQuery {
  query: string;
  setQuery: (query: string) => void;
}

export interface CommandMenuProps {
  open: boolean;
  onClose: () => void;
  /** Commands for this screen, listed first (suggestions for the selected item). */
  commands?: Command[] | undefined;
  /**
   * Offered as the last row: open the assistant ("Ask Scout"), or with a
   * query, hand it the query ("Ask Scout: <query>"); empty when none.
   */
  onAsk?: ((query: string) => void) | undefined;
  /**
   * A hook returning the section listed above the commands (conversation
   * hits). Called on every render of the open menu, so it must be a stable,
   * module-level function; return `undefined` for no section.
   */
  useSection?: ((menu: MenuQuery) => MenuSection | undefined) | undefined;
}

/** Hidden from the menu: they open or close menus themselves. */
const NOT_COMMANDS = new Set(["command.open", "overlay.close"]);

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase();

const noSection = (): undefined => undefined;

/**
 * ⌘K or `/`: search every command that is live right now (each shortcut with a
 * bound handler, from the registry) plus this screen's own commands, run one
 * with ↑ ↓ ↵, or ask the assistant. A screen can add one more section of
 * rows above them (`useSection`, e.g. matching conversations) that shares the
 * same cursor.
 */
export function CommandMenu({ open, onClose, commands = [], onAsk, useSection }: CommandMenuProps) {
  const { t } = useTranslation("ui");
  return (
    <Overlay open={open} onClose={onClose} label={t("commandMenu.label")}>
      {/* Mounted only while open, so every opening starts with an empty query. */}
      <MenuContent onClose={onClose} commands={commands} onAsk={onAsk} useSection={useSection} />
    </Overlay>
  );
}

function MenuContent({
  onClose,
  commands,
  onAsk,
  useSection,
}: {
  onClose: () => void;
  commands: Command[];
  onAsk: ((query: string) => void) | undefined;
  useSection: ((menu: MenuQuery) => MenuSection | undefined) | undefined;
}) {
  const { t } = useTranslation("ui");
  const translate = useTranslateKey();
  const { registry } = useShortcuts();
  const { definitions, isBound } = useShortcutDefinitions();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listId = useId();
  const list = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const useExtra = useSection ?? noSection;
  const section = useExtra({
    query,
    setQuery: (next) => {
      setQuery(next);
      setActive(0);
    },
  });
  const extra = section?.options ?? [];
  const typed = query.trim();

  const all = useMemo<Command[]>(() => {
    const fromRegistry = definitions
      .filter((definition) => isBound(definition.id) && !NOT_COMMANDS.has(definition.id))
      .map((definition) => ({
        id: definition.id,
        label: translate(definition.label),
        group: translate(`shortcuts:groups.${definition.group}`),
        shortcut: definition.id,
        onSelect: () => {
          registry.run(definition.id);
        },
      }));
    // Always offered: alone, it opens the assistant; with words typed, it
    // asks them.
    const ask: Command[] =
      onAsk === undefined
        ? []
        : [
            {
              id: "ask",
              label:
                typed === "" ? t("commandMenu.askAlone") : t("commandMenu.ask", { query: typed }),
              group: t("commandMenu.askGroup"),
              onSelect: () => {
                onAsk(typed);
              },
            },
          ];
    const needle = fold(typed);
    const matches = [...commands, ...fromRegistry].filter((command) =>
      fold(command.label).includes(needle),
    );
    return [...matches, ...ask];
  }, [definitions, isBound, translate, registry, commands, onAsk, typed, t]);

  // One cursor over everything: the section's rows, then the commands.
  const byGroup = new Map<string, { command: Command; index: number }[]>();
  for (const [position, command] of all.entries()) {
    const index = extra.length + position;
    byGroup.set(command.group, [...(byGroup.get(command.group) ?? []), { command, index }]);
  }
  const groups = [...byGroup];

  const total = extra.length + all.length;
  const activeIndex = Math.min(active, Math.max(total - 1, 0));
  const optionId = (index: number) => `${listId}-option-${String(index)}`;

  const run = (index: number) => {
    const option = extra[index];
    if (option !== undefined) {
      if (option.keepOpen !== true) {
        onClose();
      }
      option.onSelect();
      return;
    }
    const command = all[index - extra.length];
    if (command === undefined) {
      return;
    }
    onClose();
    command.onSelect();
  };
  const runRef = useRef(run);
  useEffect(() => {
    runRef.current = run;
  });

  const activeId = optionId(activeIndex);
  useEffect(() => {
    const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
      document.getElementById(activeId);
    element?.scrollIntoView?.({ block: "nearest" });
  }, [activeId, total]);

  // Pointer selection is delegated to the listbox (options are not focusable:
  // focus stays in the input, as the combobox pattern requires). A section
  // draws its own rows and handles their clicks itself; hovering still moves
  // the cursor.
  useEffect(() => {
    const element = list.current;
    if (element === null) {
      return;
    }
    const optionOf = (event: Event) =>
      event.target instanceof Element ? event.target.closest<HTMLElement>("[data-index]") : null;
    const onClick = (event: MouseEvent) => {
      const option = optionOf(event);
      if (option !== null && option.dataset["own"] === undefined) {
        runRef.current(Number(option.dataset["index"]));
      }
    };
    const onMove = (event: MouseEvent) => {
      const option = optionOf(event);
      if (option !== null) {
        setActive(Number(option.dataset["index"]));
      }
    };
    element.addEventListener("click", onClick);
    element.addEventListener("mousemove", onMove);
    return () => {
      element.removeEventListener("click", onClick);
      element.removeEventListener("mousemove", onMove);
    };
  }, []);

  const move = (by: number) => {
    setActive((activeIndex + by + Math.max(total, 1)) % Math.max(total, 1));
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) {
      return;
    }
    const key = event.key.toLowerCase();
    if (key === "arrowdown" || (event.ctrlKey && key === "j")) {
      event.preventDefault();
      move(1);
    } else if (key === "arrowup" || (event.ctrlKey && key === "k")) {
      event.preventDefault();
      move(-1);
    } else if (key === "enter") {
      event.preventDefault();
      const everything =
        event.metaKey || event.ctrlKey || (section?.pending === true && extra.length === 0);
      if (everything && section?.onSubmitAll !== undefined) {
        if (typed !== "") {
          onClose();
          section.onSubmitAll();
        }
      } else {
        run(activeIndex);
      }
    }
  };

  const chips = (section?.chips ?? []).map((chip) => ({
    ...chip,
    onRemove: () => {
      chip.onRemove();
      setActive(0);
      // The chip is gone: keep typing and the arrow keys where they were.
      input.current?.focus();
    },
  }));
  const hasStatus = section?.status !== undefined && section.status !== null;

  return (
    <>
      <div className={styles["inputRow"]}>
        <Visor size="md" />
        <input
          ref={input}
          data-autofocus
          className={styles["input"]}
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={total > 0 ? activeId : undefined}
          aria-label={t("commandMenu.placeholder")}
          placeholder={t("commandMenu.placeholder")}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
        <Kbd>Esc</Kbd>
      </div>
      {chips.length === 0 ? null : (
        <div className={styles["chips"]}>
          <SearchFieldChips chips={chips} />
        </div>
      )}
      <div
        ref={list}
        id={listId}
        className={styles["list"]}
        role="listbox"
        aria-label={t("commandMenu.results")}
      >
        {section === undefined ? null : (
          <div role="group" aria-label={section.group}>
            <div className={styles["groupLabel"]} aria-hidden="true">
              {section.group}
            </div>
            {section.status}
            {extra.map((option, index) => (
              <div key={option.id} data-index={index} data-own={option.render ? "" : undefined}>
                {option.render === undefined ? (
                  <div
                    id={optionId(index)}
                    className={styles["option"]}
                    role="option"
                    aria-selected={index === activeIndex}
                    tabIndex={-1}
                  >
                    <span className={styles["label"]}>{option.label}</span>
                  </div>
                ) : (
                  option.render({
                    id: optionId(index),
                    selected: index === activeIndex,
                    select: () => {
                      run(index);
                    },
                  })
                )}
              </div>
            ))}
          </div>
        )}
        {all.length === 0 && extra.length === 0 && !hasStatus ? (
          <p className={styles["empty"]}>{t("commandMenu.empty", { query })}</p>
        ) : (
          groups.map(([group, entries]) => (
            <div key={group} role="group" aria-label={group}>
              <div className={styles["groupLabel"]} aria-hidden="true">
                {group}
              </div>
              {entries.map(({ command, index }) => (
                <div
                  key={command.id}
                  id={optionId(index)}
                  data-index={index}
                  className={styles["option"]}
                  role="option"
                  aria-selected={index === activeIndex}
                  tabIndex={-1}
                >
                  <span className={styles["label"]}>{command.label}</span>
                  {command.shortcut === undefined ? null : (
                    <KbdHint shortcut={command.shortcut} withThen />
                  )}
                </div>
              ))}
            </div>
          ))
        )}
      </div>
      <div className={styles["footer"]}>
        <Hint keys="arrowup" extra="arrowdown" label={t("commandMenu.hintMove")} />
        <Hint keys="enter" label={t("commandMenu.hintOpen")} />
        {section?.onSubmitAll === undefined ? null : (
          <Hint keys="mod+enter" label={t("commandMenu.hintAll")} />
        )}
        <Hint keys="escape" label={t("commandMenu.hintClose")} />
      </div>
    </>
  );
}

function Hint({ keys, extra, label }: { keys: string; extra?: string; label: string }) {
  return (
    <Text as="span" size="xs" tone="tertiary" className={styles["hint"]}>
      <KbdHint keys={keys} />
      {extra === undefined ? null : <KbdHint keys={extra} />}
      {label}
    </Text>
  );
}
