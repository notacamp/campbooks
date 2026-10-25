import { NodeSelection } from "@tiptap/pm/state";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import { useEffect, useRef, useState, type ChangeEvent, type DragEvent } from "react";
import { useTranslation } from "react-i18next";
import type * as Y from "yjs";

import { BlockHandle, FormatBar, IconButton, RichDocument, SuggestionMenu, useUndoToast } from "~/lib/ui";
import type { SuggestionGroup } from "~/lib/ui";

import { searchMentions, type MentionCandidate } from "../api/mention-search";
import type { PageSummary, UploadedFile } from "../types";

import { EditorEnvironmentContext, type EditorEnvironment } from "./environment";
import { createExtensions } from "./extensions";
import { createMentionMenu, createSlashMenu } from "./menus";
import { filterSlash, runSlash, SLASH_ITEMS, TURN_INTO, type SlashId, type SlashItem } from "./slash";
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
}: PageEditorProps) {
  const { t } = useTranslation("pages");
  const undo = useUndoToast();
  const wrapper = useRef<HTMLDivElement>(null);
  const picker = useRef<HTMLInputElement>(null);
  const embedNext = useRef(false);
  const [images, setImages] = useState(false);
  const [handle, setHandle] = useState<Handle | undefined>();
  const [blockMenu, setBlockMenu] = useState<Handle | undefined>();

  const editorRef = useRef<ReturnType<typeof useEditor>>(null);
  const slash = useMenu<SlashItem>(() => {
    editorRef.current?.commands.focus();
  });
  const mention = useMenu<MentionCandidate>(() => {
    editorRef.current?.commands.focus();
  });

  const slashName = (id: SlashId) => t(`slash.${id}`);
  const nameRef = useRef(slashName);
  const pagesRef = useRef(pages);
  const helpersRef = useRef({
    attach: (_images: boolean) => {
      // Replaced below.
    },
    mention: (_embed: boolean) => {
      // Replaced below.
    },
    subPage: () => {
      // Replaced below.
    },
  });

  const [extensions] = useState(() => [
    createSlashMenu<SlashItem>(
      slash.bridge,
      (query) => filterSlash(SLASH_ITEMS, query, (id) => nameRef.current(id)),
      (item, range) => {
        const current = editorRef.current;
        if (current !== null) {
          runSlash(current, range, item.id, helpersRef.current);
        }
      },
    ),
    createMentionMenu<MentionCandidate>(
      mention.bridge,
      async (query, signal) =>
        (await searchMentions(environment.workspaceId, query, pagesRef.current, signal)).flatMap(
          (group) => group.candidates,
        ),
      (item, range) => {
        const current = editorRef.current;
        if (current === null) {
          return;
        }
        const embed = embedNext.current;
        embedNext.current = false;
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
    const current = editorRef.current;
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

  const editor = useEditor(
    {
      extensions: createExtensions({
        doc,
        placeholders: { paragraph: t("editor.placeholder"), heading: t("editor.heading") },
        menus: extensions,
      }),
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
    editorRef.current = editor;
    pagesRef.current = pages;
    nameRef.current = slashName;
    helpersRef.current = {
      attach: (onlyImages) => {
        setImages(onlyImages);
        requestAnimationFrame(() => {
          picker.current?.click();
        });
      },
      mention: (embed) => {
        embedNext.current = embed;
      },
      subPage: () => {
        void createSubPage().then(
          (id) => {
            editorRef.current
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
    };
  });

  const marks = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current?.isActive("bold") ?? false,
      italic: current?.isActive("italic") ?? false,
      todo: current?.isActive("taskList") ?? false,
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
    if (!editable || editor === null || box === null) {
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
    setHandle({ top: dom.getBoundingClientRect().top - box.getBoundingClientRect().top, pos: start });
  };

  const startDrag = (event: DragEvent<HTMLButtonElement>) => {
    if (editor === null || handle === undefined) {
      return;
    }
    const selection = NodeSelection.create(editor.state.doc, handle.pos);
    editor.view.dispatch(editor.state.tr.setSelection(selection));
    editor.view.dragging = { slice: selection.content(), move: true };
    event.dataTransfer.effectAllowed = "copyMove";
    event.dataTransfer.setData("text/plain", "");
    const dom = editor.view.nodeDOM(handle.pos);
    if (dom instanceof HTMLElement) {
      event.dataTransfer.setDragImage(dom, 0, 0);
    }
  };

  const addBelow = () => {
    if (editor === null || handle === undefined) {
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
    if (editor === null || blockMenu === undefined) {
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
      runSlash(editor, { from: inside, to: inside }, id as SlashId, helpersRef.current);
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
      {blockMenu === undefined || editor === null ? null : (
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
              onPress: () => editor?.chain().focus().insertContent("/").run(),
            },
            {
              id: "mention",
              label: t("format.mention"),
              glyph: "@",
              onPress: () => editor?.chain().focus().insertContent("@").run(),
            },
            {
              id: "todo",
              label: t("format.todo"),
              icon: "check",
              pressed: marks?.todo ?? false,
              onPress: () => editor?.chain().focus().toggleTaskList().run(),
            },
            {
              id: "bold",
              label: t("format.bold"),
              icon: "bold",
              pressed: marks?.bold ?? false,
              onPress: () => editor?.chain().focus().toggleBold().run(),
            },
            {
              id: "italic",
              label: t("format.italic"),
              icon: "italic",
              pressed: marks?.italic ?? false,
              onPress: () => editor?.chain().focus().toggleItalic().run(),
            },
          ]}
          end={
            <IconButton
              icon="chevron-down"
              size="lg"
              label={t("format.hide")}
              onClick={() => editor?.commands.blur()}
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
