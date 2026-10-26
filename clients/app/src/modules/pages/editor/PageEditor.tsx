import type { Editor } from "@tiptap/core";
import { NodeSelection } from "@tiptap/pm/state";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import { useEffect, useMemo, useRef, useState, type ChangeEvent, type DragEvent } from "react";
import { useTranslation } from "react-i18next";
import type * as Y from "yjs";

import {
  BlockHandle,
  FormatBar,
  IconButton,
  RichDocument,
  SuggestionMenu,
  useUndoToast,
} from "~/lib/ui";
import type { SuggestionGroup } from "~/lib/ui";

import { searchMentions, type MentionCandidate } from "../api/mention-search";
import type { PageSummary, UploadedFile } from "../types";

import { EditorEnvironmentContext, type EditorEnvironment } from "./environment";
import { createExtensions } from "./extensions";
import { createMentionMenu, createSlashMenu } from "./menus";
import {
  filterSlash,
  runSlash,
  SLASH_ITEMS,
  TABLE_SLASH_IDS,
  TURN_INTO,
  type SlashHelpers,
  type SlashId,
  type SlashItem,
} from "./slash";
import { Latest } from "./latest";
import { useMenu } from "./use-menu";

export interface PageEditorProps {
  environment: EditorEnvironment;
  doc: Y.Doc;
  editable: boolean;
  /** Every page the caller may open: found by `@`. */
  pages: readonly PageSummary[];
  /** Store a file on the page. */
  upload: (file: File) => Promise<UploadedFile>;
  /** Make a sub-page under this page; its id. */
  createSubPage: () => Promise<string>;
  /** Pages have sub-pages; the row of a table does not. Default true. */
  subPages?: boolean | undefined;
  /** What the table blocks need; none while tables are off, and the blocks are not offered. */
  tables?: TableHelpers | undefined;
}

/** What the editor asks of tables (the `/table` and `/link to table` blocks). */
export interface TableHelpers {
  /** Make a table under this page: its id and its first view. */
  create: () => Promise<{ table: string; view: string }>;
  /** Put a table just made in the trash (the Undo of making it). */
  discard: (table: string) => Promise<void>;
  /** A table's first view. */
  firstView: (table: string) => Promise<string>;
  /** Whether a page is a table the viewer may open. */
  isTable: (page: string) => boolean;
}

const IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);

/** A block the pointer is beside. */
interface Handle {
  top: number;
  pos: number;
}

/**
 * A page's body: the editor over a Yjs document, the `/` and `@` menus, the
 * block handle with its menu, file and image upload, and the phone's
 * formatting bar. A member who may only read gets the same page without any
 * of it editable.
 */
export function PageEditor({
  environment,
  doc,
  editable,
  pages,
  upload,
  createSubPage,
  subPages = true,
  tables,
}: PageEditorProps) {
  const { t } = useTranslation("pages");
  const undo = useUndoToast();
  const wrapper = useRef<HTMLDivElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const [embedNext] = useState(() => new Latest(false));
  const [images, setImages] = useState(false);
  const [handle, setHandle] = useState<Handle | undefined>();
  const [blockMenu, setBlockMenu] = useState<Handle | undefined>();

  const [editorBox] = useState(() => new Latest<Editor | null>(null));
  // The `@` menu is open for a table (`/link to table`), not for a mention.
  const [tableNext] = useState(() => new Latest(false));
  const slash = useMenu<SlashItem>(() => {
    editorBox.current?.commands.focus();
  });
  const mention = useMenu<MentionCandidate>(() => {
    tableNext.set(false);
    editorBox.current?.commands.focus();
  });

  // The `@` menu closing ends a link to a table.
  const mentionClosed = mention.menu === undefined;
  useEffect(() => {
    if (mentionClosed) {
      tableNext.set(false);
    }
  }, [mentionClosed, tableNext]);

  const slashName = (id: SlashId) => t(`slash.${id}`);
  const [nameBox] = useState(() => new Latest(slashName));
  const [pagesBox] = useState(() => new Latest(pages));
  const idle = (): void => {
    // Nothing to do until the editor has mounted.
  };
  const [helpersBox] = useState(
    () =>
      new Latest<SlashHelpers>({
        attach: idle,
        mention: idle,
        subPage: idle,
        table: idle,
        tableLink: idle,
      }),
  );
  // What the menus offer: no table blocks while tables are off, no sub-pages in a row.
  const [offerBox] = useState(() => new Latest({ subPages, tables: tables !== undefined }));
  const [tablesBox] = useState(() => new Latest<TableHelpers | undefined>(tables));

  const [extensions] = useState(() => [
    createSlashMenu<SlashItem>(
      slash.bridge,
      (query) =>
        filterSlash(
          SLASH_ITEMS.filter(
            (item) =>
              (offerBox.current.subPages || item.id !== "subpage") &&
              (offerBox.current.tables || !TABLE_SLASH_IDS.includes(item.id)),
          ),
          query,
          (id) => nameBox.current(id),
        ),
      (item, range) => {
        const current = editorBox.current;
        if (current !== null) {
          runSlash(current, range, item.id, helpersBox.current);
        }
      },
    ),
    createMentionMenu<MentionCandidate>(
      mention.bridge,
      async (query, signal) => {
        const found = (
          await searchMentions(environment.workspaceId, query, pagesBox.current, signal)
        ).flatMap((group) => group.candidates);
        // Linking to a table offers only the tables the viewer may open.
        return tableNext.current
          ? found.filter(
              (item) => item.type === "pages.page" && tablesBox.current?.isTable(item.id) === true,
            )
          : found;
      },
      (item, range) => {
        const current = editorBox.current;
        if (current === null) {
          return;
        }
        if (tableNext.current) {
          tableNext.set(false);
          const tablesHelper = tablesBox.current;
          current.chain().focus().deleteRange(range).run();
          void tablesHelper?.firstView(item.id).then(
            (view) => {
              editorBox.current
                ?.chain()
                .focus()
                .insertContent({ type: "tableView", attrs: { table: item.id, view } })
                .run();
            },
            () => {
              undo.fail({ label: t("tables.toast.linkFailed") });
            },
          );
          return;
        }
        const embed = embedNext.current;
        embedNext.set(false);
        const node = {
          type: embed ? "embed" : "mention",
          attrs: { ref_type: item.type, ref_id: item.id },
        };
        current
          .chain()
          .focus()
          .insertContentAt(range, embed ? [node] : [node, { type: "text", text: " " }])
          .run();
      },
    ),
  ]);

  const insertFile = async (file: File) => {
    const current = editorBox.current;
    try {
      const stored = await upload(file);
      current
        ?.chain()
        .focus()
        .insertContent({
          type: IMAGE_TYPES.has(file.type) ? "image" : "file",
          attrs: { blob: stored.blob, name: file.name, size: stored.size },
        })
        .run();
    } catch {
      undo.fail({ label: t("upload.failed", { name: file.name }) });
    }
  };

  // Made once per document (and language). New extension objects on every
  // render make the editor rebuild its plugins when it re-renders (the save
  // status changes as you type), which drops the caret to the start.
  const placeholder = t("editor.placeholder");
  const headingPlaceholder = t("editor.heading");
  const editorExtensions = useMemo(
    () =>
      createExtensions({
        doc,
        placeholders: { paragraph: placeholder, heading: headingPlaceholder },
        menus: extensions,
      }),
    [doc, extensions, placeholder, headingPlaceholder],
  );
  const editor = useEditor(
    {
      extensions: editorExtensions,
      editable,
      editorProps: {
        attributes: { "aria-label": t("editor.label"), role: "textbox", "aria-multiline": "true" },
        handlePaste: (_view, event) => {
          const files = [...(event.clipboardData?.files ?? [])];
          if (files.length === 0) {
            return false;
          }
          for (const file of files) {
            void insertFile(file);
          }
          return true;
        },
        handleDrop: (_view, event) => {
          const files = [...(event.dataTransfer?.files ?? [])];
          if (files.length === 0) {
            return false;
          }
          for (const file of files) {
            void insertFile(file);
          }
          return true;
        },
      },
    },
    [doc],
  );

  // The latest of everything the menus' callbacks reach for.
  useEffect(() => {
    editorBox.set(editor);
    pagesBox.set(pages);
    nameBox.set(slashName);
    offerBox.set({ subPages, tables: tables !== undefined });
    tablesBox.set(tables);
    helpersBox.set({
      table: () => {
        const made = tablesBox.current?.create();
        if (made === undefined) {
          return;
        }
        void made.then(
          ({ table, view }) => {
            editorBox.current
              ?.chain()
              .focus()
              .insertContent({ type: "tableView", attrs: { table, view } })
              .run();
            undo.push({
              label: t("tables.toast.inlineCreated"),
              undo: () => {
                const current = editorBox.current;
                current?.state.doc.descendants((node, pos) => {
                  if (node.type.name === "tableView" && node.attrs["table"] === table) {
                    current
                      .chain()
                      .focus()
                      .deleteRange({ from: pos, to: pos + node.nodeSize })
                      .run();
                    return false;
                  }
                  return true;
                });
                void tablesBox.current?.discard(table);
              },
            });
          },
          () => {
            undo.fail({ label: t("tables.toast.createFailed") });
          },
        );
      },
      tableLink: () => {
        tableNext.set(true);
      },
      attach: (onlyImages) => {
        setImages(onlyImages);
        requestAnimationFrame(() => {
          picker.current?.click();
        });
      },
      mention: (embed) => {
        embedNext.set(embed);
      },
      subPage: () => {
        void createSubPage().then(
          (id) => {
            editorBox.current
              ?.chain()
              .focus()
              .insertContent({ type: "pageLink", attrs: { page: id } })
              .run();
          },
          () => {
            undo.fail({ label: t("subpage.failed") });
          },
        );
      },
    });
  });

  const marks = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      todo: current.isActive("taskList"),
    }),
  });

  const onFiles = (event: ChangeEvent<HTMLInputElement>) => {
    const files = [...(event.target.files ?? [])];
    event.target.value = "";
    for (const file of files) {
      void insertFile(file);
    }
  };

  const follow = (event: { clientX: number; clientY: number }) => {
    const box = wrapper.current;
    if (!editable || box === null) {
      return;
    }
    const content = editor.view.dom.getBoundingClientRect();
    const found = editor.view.posAtCoords({ left: content.left + 8, top: event.clientY });
    if (found === null) {
      setHandle(undefined);
      return;
    }
    const $pos = editor.state.doc.resolve(found.pos);
    const start = $pos.depth >= 1 ? $pos.before(1) : found.inside;
    const dom = start < 0 ? null : editor.view.nodeDOM(start);
    if (!(dom instanceof HTMLElement)) {
      setHandle(undefined);
      return;
    }
    setHandle({
      top: dom.getBoundingClientRect().top - box.getBoundingClientRect().top,
      pos: start,
    });
  };

  const startDrag = (event: DragEvent<HTMLButtonElement>) => {
    if (handle === undefined) {
      return;
    }
    const selection = NodeSelection.create(editor.state.doc, handle.pos);
    editor.view.dispatch(editor.state.tr.setSelection(selection));
    const view = editorBox.current?.view;
    if (view !== undefined) {
      Object.assign(view, { dragging: { slice: selection.content(), move: true } });
    }
    event.dataTransfer.effectAllowed = "copyMove";
    event.dataTransfer.setData("text/plain", "");
    const dom = editor.view.nodeDOM(handle.pos);
    if (dom instanceof HTMLElement) {
      event.dataTransfer.setDragImage(dom, 0, 0);
    }
  };

  const addBelow = () => {
    if (handle === undefined) {
      return;
    }
    const node = editor.state.doc.nodeAt(handle.pos);
    const end = handle.pos + (node?.nodeSize ?? 0);
    editor
      .chain()
      .focus()
      .insertContentAt(end, { type: "paragraph" })
      .setTextSelection(end + 1)
      .insertContent("/")
      .run();
  };

  const pickBlock = (id: string) => {
    if (blockMenu === undefined) {
      return;
    }
    const node = editor.state.doc.nodeAt(blockMenu.pos);
    setBlockMenu(undefined);
    if (node === null) {
      return;
    }
    if (id === "duplicate") {
      editor
        .chain()
        .focus()
        .insertContentAt(blockMenu.pos + node.nodeSize, node.toJSON() as Record<string, unknown>)
        .run();
    } else if (id === "delete") {
      editor
        .chain()
        .focus()
        .deleteRange({ from: blockMenu.pos, to: blockMenu.pos + node.nodeSize })
        .run();
    } else {
      const inside = blockMenu.pos + 1;
      editor.chain().focus().setTextSelection(inside).run();
      runSlash(editor, { from: inside, to: inside }, id as SlashId, helpersBox.current);
    }
  };

  const slashGroups: SuggestionGroup[] = (["basic", "campbooks"] as const).map((group) => ({
    id: group,
    label: t(`slash.group.${group}`),
    items: (slash.menu?.items ?? [])
      .filter((item) => item.group === group)
      .map((item) => ({
        id: item.id,
        label: slashName(item.id),
        icon: item.icon,
        glyph: item.glyph,
        hint: item.hint,
      })),
  }));
  const slashItems = slash.menu?.items ?? [];
  const activeSlash = slashItems[slash.active]?.id;

  const mentionItems = mention.menu?.items ?? [];
  const mentionKey = (item: MentionCandidate) => `${item.type}:${item.id}`;
  const mentionGroups: SuggestionGroup[] = MENTION_KINDS.map((kind) => ({
    id: kind,
    label: t(`mention.group.${kind}`),
    items: mentionItems
      .filter((item) => KIND_OF[item.type] === kind)
      .map((item) => ({
        id: mentionKey(item),
        label: item.label === "" ? t("untitled") : item.label,
        detail: item.detail,
        icon: KIND_ICON[kind],
      })),
  }));
  const activeMention = mentionItems[mention.active];

  return (
    <EditorEnvironmentContext value={environment}>
      <div
        ref={wrapper}
        className="relative"
        onMouseMove={follow}
        onMouseLeave={() => {
          setHandle(undefined);
        }}
        role="presentation"
      >
        {editable && handle !== undefined ? (
          <BlockHandle
            top={handle.top}
            addLabel={t("block.add")}
            onAdd={addBelow}
            menuLabel={t("block.menu")}
            onMenu={() => {
              setBlockMenu(handle);
            }}
            onDragStart={startDrag}
          />
        ) : null}
        <RichDocument>
          <EditorContent editor={editor} />
        </RichDocument>
      </div>
      <input
        ref={picker}
        type="file"
        hidden
        multiple
        accept={images ? "image/png,image/jpeg,image/gif,image/webp" : undefined}
        aria-label={t(images ? "slash.image" : "slash.file")}
        onChange={onFiles}
      />
      {slash.menu === undefined ? null : (
        <SuggestionMenu
          label={t("slash.label")}
          groups={slashGroups}
          active={activeSlash}
          anchor={slash.menu.anchor}
          empty={t("slash.empty")}
          onPick={(id) => {
            const item = slashItems.find((candidate) => candidate.id === id);
            if (item !== undefined) {
              slash.menu?.command(item);
            }
          }}
        />
      )}
      {mention.menu === undefined ? null : (
        <SuggestionMenu
          label={t("mention.label")}
          groups={mentionGroups}
          active={activeMention === undefined ? undefined : mentionKey(activeMention)}
          anchor={mention.menu.anchor}
          empty={t("mention.empty")}
          loading={mention.menu.loading ? t("mention.searching") : undefined}
          onPick={(id) => {
            const item = mentionItems.find((candidate) => mentionKey(candidate) === id);
            if (item !== undefined) {
              mention.menu?.command(item);
            }
          }}
        />
      )}
      {blockMenu === undefined ? null : (
        <SuggestionMenu
          label={t("block.menu")}
          groups={[
            {
              id: "turn",
              label: t("block.turnInto"),
              items: TURN_INTO.map((id) => ({ id, label: slashName(id) })),
            },
            {
              id: "block",
              label: t("block.block"),
              items: [
                { id: "duplicate", label: t("block.duplicate"), icon: "copy" },
                { id: "delete", label: t("block.delete"), icon: "trash" },
              ],
            },
          ]}
          active={undefined}
          anchor={{
            x: editor.view.coordsAtPos(blockMenu.pos).left,
            y: editor.view.coordsAtPos(blockMenu.pos).bottom,
          }}
          empty=""
          onPick={pickBlock}
        />
      )}
      {editable ? (
        <FormatBar
          label={t("format.label")}
          items={[
            {
              id: "block",
              label: t("format.block"),
              glyph: "/",
              onPress: () => editor.chain().focus().insertContent("/").run(),
            },
            {
              id: "mention",
              label: t("format.mention"),
              glyph: "@",
              onPress: () => editor.chain().focus().insertContent("@").run(),
            },
            {
              id: "todo",
              label: t("format.todo"),
              icon: "check",
              pressed: marks.todo,
              onPress: () => editor.chain().focus().toggleTaskList().run(),
            },
            {
              id: "bold",
              label: t("format.bold"),
              icon: "bold",
              pressed: marks.bold,
              onPress: () => editor.chain().focus().toggleBold().run(),
            },
            {
              id: "italic",
              label: t("format.italic"),
              icon: "italic",
              pressed: marks.italic,
              onPress: () => editor.chain().focus().toggleItalic().run(),
            },
          ]}
          end={
            <IconButton
              icon="chevron-down"
              size="lg"
              label={t("format.hide")}
              onClick={() => editor.commands.blur()}
            />
          }
        />
      ) : null}
    </EditorEnvironmentContext>
  );
}

const MENTION_KINDS = ["people", "threads", "tasks", "events", "pages"] as const;

const KIND_OF = {
  "workspaces.member": "people",
  person: "people",
  "conversations.conversation": "threads",
  "tasks.task": "tasks",
  "schedule.event": "events",
  "pages.page": "pages",
} as const;

const KIND_ICON = {
  people: "people",
  threads: "mail",
  tasks: "tasks",
  events: "event",
  pages: "document",
} as const;
