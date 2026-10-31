import { closeHistory } from "@tiptap/pm/history";
import type { Editor } from "@tiptap/react";
import { useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";

import { Button, Icon, IconButton, Popover } from "@notacamp/ui";
import { AskLine } from "../ReplyDraft/ReplyDraft";

import type { ComposerMode, ComposerModes, ComposerScout, ScoutRewrite } from "./composer-types";
import type { DocNode } from "./marks";
import styles from "./ComposerMenus.module.css";

/** Longest change Scout is asked for, in characters (the server's limit). */
const MAX_ASK_CHARS = 300;

const MODES: readonly { mode: ComposerMode; keys: string; key: string }[] = [
  { mode: "reply", keys: "r", key: "r" },
  { mode: "reply_all", keys: "shift+r", key: "R" },
  { mode: "forward", keys: "f", key: "f" },
];

/**
 * The title's menu: Reply, Reply all, Forward. Its keys (R, ⇧R, F) are the
 * reader's own, live only while the menu is open.
 */
export function ComposerHeaderMenus({ modes }: { modes: ComposerModes }) {
  const { t } = useTranslation("ui");
  return (
    <Popover
      label={t("composer.mode.label")}
      layer="top"
      placement="bottom-start"
      renderTrigger={(trigger) => (
        <IconButton {...trigger} label={t("composer.mode.label")} icon="chevron-down" />
      )}
    >
      {(close) => (
        <div
          className={styles["menu"]}
          role="menu"
          tabIndex={-1}
          aria-label={t("composer.mode.label")}
          onKeyDown={(event: KeyboardEvent) => {
            const found = MODES.find((m) => m.key === event.key);
            if (found !== undefined && !event.metaKey && !event.ctrlKey && !event.altKey) {
              event.preventDefault();
              close();
              if (found.mode !== modes.value) {
                modes.onChange(found.mode);
              }
            }
          }}
        >
          {MODES.map(({ mode, keys }) => (
            <Button
              key={mode}
              variant="ghost"
              keys={keys}
              role="menuitemradio"
              aria-checked={mode === modes.value}
              onClick={() => {
                close();
                if (mode !== modes.value) {
                  modes.onChange(mode);
                }
              }}
            >
              {t(`composer.mode.${mode}`)}
            </Button>
          ))}
        </div>
      )}
    </Popover>
  );
}

/**
 * `text` as a document of plain paragraphs: blank lines separate paragraphs,
 * single line breaks stay line breaks. Every piece is a text node with no
 * marks, so nothing in it is read as formatting, HTML or a link.
 */
function plainDoc(text: string): DocNode {
  const paragraphs = text
    .replaceAll("\r\n", "\n")
    .trim()
    .split(/\n{2,}/u)
    .map((paragraph): DocNode => {
      const content: DocNode[] = [];
      for (const [at, line] of paragraph.split("\n").entries()) {
        if (at > 0) {
          content.push({ type: "hardBreak" });
        }
        if (line !== "") {
          content.push({ type: "text", text: line });
        }
      }
      return content.length === 0 ? { type: "paragraph" } : { type: "paragraph", content };
    });
  return { type: "doc", content: paragraphs.length === 0 ? [{ type: "paragraph" }] : paragraphs };
}

/**
 * Replace the editor's text with `text`, as plain text nodes built from the
 * schema (never `insertContent(string)`, which parses HTML, and never
 * `fromMarks`). Autolinking is off for it, and it is its own undo step.
 */
function putPlainText(editor: Editor, text: string): void {
  const doc = editor.schema.nodeFromJSON(plainDoc(text));
  const tr = closeHistory(
    editor.state.tr
      .replaceWith(0, editor.state.doc.content.size, doc.content)
      .setMeta("preventAutolink", true),
  );
  editor.view.dispatch(closeHistory(tr));
}

export interface ScoutMenuProps {
  scout: ComposerScout;
  /** Focus is inside the composer: ⌘J works. */
  active: boolean;
  editor: Editor;
  writing: boolean;
  onWriting: (writing: boolean) => void;
}

/**
 * Scout ▾ (⌘J): Shorter, Warmer, Fix spelling and Change…, each rewriting
 * the text in place while the text is read-only, with a toast whose Undo
 * (and the editor's own ⌘Z) puts the previous text back.
 */
export function ScoutMenu({ scout, active, editor, writing, onWriting }: ScoutMenuProps) {
  const { t } = useTranslation("ui");
  const [open, setOpen] = useState(false);

  useShortcut(
    "compose.draft",
    () => {
      setOpen(!open);
    },
    { enabled: active && !writing },
  );

  async function run(how: ScoutRewrite) {
    const plain = editor.state.doc.textBetween(0, editor.state.doc.content.size, "\n", "\n");
    if (plain.trim() === "" || writing) {
      return;
    }
    setOpen(false);
    onWriting(true);
    editor.setEditable(false);
    try {
      const text = await scout.onRewrite(how, plain);
      if (text !== undefined && text.trim() !== "") {
        putPlainText(editor, text);
        scout.onDone?.(() => {
          editor.chain().focus().undo().run();
        });
      }
    } finally {
      editor.setEditable(true);
      onWriting(false);
    }
  }

  return (
    <Popover
      label={t("composer.scout.menu")}
      layer="top"
      placement="bottom-end"
      open={open}
      onOpenChange={setOpen}
      renderTrigger={(trigger) => (
        <Button
          {...trigger}
          variant="ghost"
          ai
          shortcut="compose.draft"
          disabled={writing}
          aria-label={t("composer.scout.menu")}
        >
          <span className={styles["scoutLabel"]}>
            {t("composer.scout.label")}
            <Icon name="chevron-down" />
          </span>
        </Button>
      )}
    >
      {(close) => (
        <div className={styles["menu"]}>
          {(
            [
              ["shorter", { kind: "shorter" }],
              ["warmer", { kind: "warmer" }],
              ["spelling", { kind: "spelling" }],
            ] as const
          ).map(([id, how]) => (
            <Button
              key={id}
              variant="ghost"
              onClick={() => {
                close();
                void run(how);
              }}
            >
              {t(`composer.scout.${id}`)}
            </Button>
          ))}
          <AskLine
            label={t("composer.scout.change.label")}
            placeholder={t("composer.scout.change.placeholder")}
            action={t("composer.scout.change.action")}
            maxLength={MAX_ASK_CHARS}
            onSubmit={(change) => {
              close();
              void run({ kind: "change", change });
            }}
          />
        </div>
      )}
    </Popover>
  );
}
