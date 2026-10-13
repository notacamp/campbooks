import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcutDefinitions, useShortcuts, type ShortcutId } from "~/lib/shortcuts";

import { Overlay } from "../../internal/Overlay";
import { useTranslateKey } from "../../internal/translate-key";
import { Kbd, KbdHint } from "../Kbd/Kbd";
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

export interface CommandMenuProps {
  open: boolean;
  onClose: () => void;
  /** Commands for this screen, listed first (suggestions for the selected item). */
  commands?: Command[] | undefined;
  /** Offered as the last row while there is a query: hand it to the assistant. */
  onAsk?: ((query: string) => void) | undefined;
}

/** Hidden from the menu: they open or close menus themselves. */
const NOT_COMMANDS = new Set(["command.open", "overlay.close"]);

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase();

/**
 * ⌘K: search every command that is live right now (each shortcut with a
 * bound handler, from the registry) plus this screen's own commands, run one
 * with ↑ ↓ ↵, or ask the assistant.
 */
export function CommandMenu({ open, onClose, commands = [], onAsk }: CommandMenuProps) {
  const { t } = useTranslation("ui");
  return (
    <Overlay open={open} onClose={onClose} label={t("commandMenu.label")}>
      {/* Mounted only while open, so every opening starts with an empty query. */}
      <MenuContent onClose={onClose} commands={commands} onAsk={onAsk} />
    </Overlay>
  );
}

function MenuContent({
  onClose,
  commands,
  onAsk,
}: {
  onClose: () => void;
  commands: Command[];
  onAsk: ((query: string) => void) | undefined;
}) {
  const { t } = useTranslation("ui");
  const translate = useTranslateKey();
  const { registry } = useShortcuts();
  const { definitions, isBound } = useShortcutDefinitions();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listId = useId();
  const list = useRef<HTMLDivElement>(null);

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
    const ask: Command[] =
      onAsk !== undefined && query.trim() !== ""
        ? [
            {
              id: "ask",
              label: t("commandMenu.ask", { query: query.trim() }),
              group: t("commandMenu.askGroup"),
              onSelect: () => {
                onAsk(query.trim());
              },
            },
          ]
        : [];
    const needle = fold(query.trim());
    const matches = [...commands, ...fromRegistry].filter((command) =>
      fold(command.label).includes(needle),
    );
    return [...matches, ...ask];
  }, [definitions, isBound, translate, registry, commands, onAsk, query, t]);

  const groups = useMemo(() => {
    const byGroup = new Map<string, { command: Command; index: number }[]>();
    for (const [index, command] of all.entries()) {
      byGroup.set(command.group, [...(byGroup.get(command.group) ?? []), { command, index }]);
    }
    return [...byGroup];
  }, [all]);

  const activeIndex = Math.min(active, Math.max(all.length - 1, 0));
  const optionId = (index: number) => `${listId}-option-${String(index)}`;

  const run = (index: number) => {
    const command = all[index];
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

  // Pointer selection is delegated to the listbox (options are not focusable:
  // focus stays in the input, as the combobox pattern requires).
  useEffect(() => {
    const element = list.current;
    if (element === null) {
      return;
    }
    const indexOf = (event: Event) => {
      const option =
        event.target instanceof Element ? event.target.closest<HTMLElement>("[data-index]") : null;
      return option === null ? null : Number(option.dataset["index"]);
    };
    const onClick = (event: MouseEvent) => {
      const index = indexOf(event);
      if (index !== null) {
        runRef.current(index);
      }
    };
    const onMove = (event: MouseEvent) => {
      const index = indexOf(event);
      if (index !== null) {
        setActive(index);
      }
    };
    element.addEventListener("click", onClick);
    element.addEventListener("mousemove", onMove);
    return () => {
      element.removeEventListener("click", onClick);
      element.removeEventListener("mousemove", onMove);
    };
  }, []);

  return (
    <>
      <div className={styles["inputRow"]}>
        <Visor size="md" />
        <input
          data-autofocus
          className={styles["input"]}
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={all.length > 0 ? optionId(activeIndex) : undefined}
          aria-label={t("commandMenu.placeholder")}
          placeholder={t("commandMenu.placeholder")}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={(event) => {
            const count = Math.max(all.length, 1);
            switch (event.key) {
              case "ArrowDown": {
                event.preventDefault();
                setActive((activeIndex + 1) % count);
                break;
              }
              case "ArrowUp": {
                event.preventDefault();
                setActive((activeIndex - 1 + count) % count);
                break;
              }
              case "Enter": {
                event.preventDefault();
                run(activeIndex);
                break;
              }
              default:
            }
          }}
        />
        <Kbd>Esc</Kbd>
      </div>
      <div
        ref={list}
        id={listId}
        className={styles["list"]}
        role="listbox"
        aria-label={t("commandMenu.results")}
      >
        {all.length === 0 ? (
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
    </>
  );
}
