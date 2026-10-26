import type { Editor } from "@tiptap/core";

import type { IconName } from "~/lib/ui";

/** Every block the `/` menu can insert, by id (their names are `slash.<id>` in the messages). */
export const SLASH_IDS = [
  "text",
  "heading1",
  "heading2",
  "heading3",
  "todo",
  "bullets",
  "numbers",
  "toggle",
  "quote",
  "callout",
  "divider",
  "code",
  "mention",
  "embed",
  "subpage",
  "file",
  "image",
  "table",
  "tableLink",
] as const;

export type SlashId = (typeof SLASH_IDS)[number];

/** What the page lends the blocks that reach outside the text. */
export interface SlashHelpers {
  /** Open the file picker; images only, or any file. */
  attach: (images: boolean) => void;
  /** Open the `@` menu; its pick becomes a block that shows the record, not a chip. */
  mention: (embed: boolean) => void;
  /** Make a sub-page under this page and put a link to it here. */
  subPage: () => void;
  /** Make a table in this page, as an inline block. */
  table: () => void;
  /** Open the `@` menu for tables only; its pick becomes an inline block of that table. */
  tableLink: () => void;
}

/** The blocks that need tables to be on. */
export const TABLE_SLASH_IDS: readonly SlashId[] = ["table", "tableLink"];

export interface SlashItem {
  id: SlashId;
  group: "basic" | "campbooks";
  icon?: IconName | undefined;
  glyph?: string | undefined;
  /** The Markdown shortcut that makes the same block. */
  hint?: string | undefined;
}

export const SLASH_ITEMS: readonly SlashItem[] = [
  { id: "text", group: "basic", glyph: "T" },
  { id: "heading1", group: "basic", glyph: "H1", hint: "#" },
  { id: "heading2", group: "basic", glyph: "H2", hint: "##" },
  { id: "heading3", group: "basic", glyph: "H3", hint: "###" },
  { id: "todo", group: "basic", icon: "check", hint: "[]" },
  { id: "bullets", group: "basic", icon: "bullets", hint: "-" },
  { id: "numbers", group: "basic", icon: "numbers", hint: "1." },
  { id: "toggle", group: "basic", icon: "chevron-right", hint: ">>" },
  { id: "quote", group: "basic", glyph: "“", hint: ">" },
  { id: "callout", group: "basic", glyph: "!" },
  { id: "divider", group: "basic", glyph: "—", hint: "---" },
  { id: "code", group: "basic", glyph: "<>", hint: "```" },
  { id: "mention", group: "campbooks", glyph: "@", hint: "@" },
  { id: "embed", group: "campbooks", icon: "mail" },
  { id: "subpage", group: "campbooks", icon: "document" },
  { id: "file", group: "campbooks", icon: "attachment" },
  { id: "image", group: "campbooks", icon: "image" },
  { id: "table", group: "campbooks", icon: "table" },
  { id: "tableLink", group: "campbooks", icon: "link" },
];

/** The blocks a block can be turned into, from its menu. */
export const TURN_INTO: readonly SlashId[] = [
  "text",
  "heading1",
  "heading2",
  "heading3",
  "todo",
  "bullets",
  "numbers",
  "toggle",
  "quote",
  "callout",
  "code",
];

/**
 * Make the block `id` at `range` (the slash and what was typed after it, or
 * an empty range at the start of the block being turned into another).
 */
export function runSlash(
  editor: Editor,
  range: { from: number; to: number },
  id: SlashId,
  helpers: SlashHelpers,
): void {
  const chain = () => editor.chain().focus().deleteRange(range);
  switch (id) {
    case "text": {
      chain().setParagraph().run();
      return;
    }
    case "heading1":
    case "heading2":
    case "heading3": {
      chain()
        .setNode("heading", { level: Number(id.slice(-1)) })
        .run();
      return;
    }
    case "todo": {
      chain().toggleTaskList().run();
      return;
    }
    case "bullets": {
      chain().toggleBulletList().run();
      return;
    }
    case "numbers": {
      chain().toggleOrderedList().run();
      return;
    }
    case "toggle": {
      chain().wrapIn("toggle", { open: true }).run();
      return;
    }
    case "quote": {
      chain().toggleBlockquote().run();
      return;
    }
    case "callout": {
      chain().wrapIn("callout", { tone: "info" }).run();
      return;
    }
    case "divider": {
      chain().setHorizontalRule().run();
      return;
    }
    case "code": {
      chain().setCodeBlock().run();
      return;
    }
    case "mention": {
      chain().insertContent("@").run();
      helpers.mention(false);
      return;
    }
    case "embed": {
      chain().insertContent("@").run();
      helpers.mention(true);
      return;
    }
    case "subpage": {
      chain().run();
      helpers.subPage();
      return;
    }
    case "file":
    case "image": {
      chain().run();
      helpers.attach(id === "image");
      return;
    }
    case "table": {
      chain().run();
      helpers.table();
      return;
    }
    case "tableLink": {
      chain().insertContent("@").run();
      helpers.tableLink();
    }
  }
}

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

/** The items whose name (as the member reads it) contains what they typed. */
export function filterSlash(
  items: readonly SlashItem[],
  query: string,
  name: (id: SlashId) => string,
): SlashItem[] {
  const typed = fold(query.trim());
  return items.filter((item) => typed === "" || fold(name(item.id)).includes(typed));
}
