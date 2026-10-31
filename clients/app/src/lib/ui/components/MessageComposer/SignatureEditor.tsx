import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { IconButton } from "@notacamp/ui";

import { composerExtensions } from "./editor";
import { LinkForm } from "./LinkForm";
import { fromMarks, toMarks, type DocNode } from "./marks";
import styles from "./SignatureEditor.module.css";

export interface SignatureEditorProps {
  /** What the editor is for: "Your signature for sam@example.com". */
  label: string;
  /** The signature as marks, when the editor opens; it is not set again. */
  initial: string;
  onChange: (marks: string) => void;
  /** ⌘↵ in the text. */
  onSubmit: () => void;
}

/**
 * A small editor for a signature: the composer's schema with bold, italic
 * and links only (the same marks a message carries), lines and paragraphs.
 * It hands back marks. ⌘↵ submits, ⌘B and ⌘I format.
 */
export function SignatureEditor({ label, initial, onChange, onSubmit }: SignatureEditorProps) {
  const { t } = useTranslation("ui");
  const id = useId();
  const changed = useRef(onChange);
  const submitted = useRef(onSubmit);
  useEffect(() => {
    changed.current = onChange;
    submitted.current = onSubmit;
  });
  const [link, setLink] = useState<{ from: number; to: number; words: string; href: string }>();
  const editor = useEditor({
    extensions: composerExtensions({ compact: true }),
    content: fromMarks(initial),
    editorProps: {
      attributes: {
        id: `${id}-text`,
        role: "textbox",
        "aria-multiline": "true",
        "aria-label": label,
        class: styles["editor"] ?? "",
      },
    },
    onUpdate: ({ editor: updated }) => {
      changed.current(toMarks(updated.getJSON() as DocNode));
    },
  });
  useEffect(() => {
    Object.assign((editor.storage as unknown as Record<string, object>)["composerSend"] ?? {}, {
      send: () => {
        submitted.current();
      },
    });
  }, [editor]);
  const marks = useEditorState({
    editor,
    selector: ({ editor: current }) => ({
      bold: current.isActive("bold"),
      italic: current.isActive("italic"),
      link: current.isActive("link"),
    }),
  });

  function openLink() {
    if (editor.isActive("link")) {
      editor.chain().extendMarkRange("link").run();
    }
    const { from, to } = editor.state.selection;
    const href: unknown = editor.isActive("link") ? editor.getAttributes("link")["href"] : "";
    setLink({
      from,
      to,
      words: editor.state.doc.textBetween(from, to, " "),
      href: typeof href === "string" ? href : "",
    });
  }

  return (
    <div className={styles["box"]}>
      <div className={styles["toolbar"]} role="toolbar" aria-label={t("composer.formatting")}>
        <IconButton
          label={t("composer.format.bold")}
          icon="bold"
          aria-pressed={marks.bold}
          onClick={() => {
            editor.chain().focus().toggleBold().run();
          }}
        />
        <IconButton
          label={t("composer.format.italic")}
          icon="italic"
          aria-pressed={marks.italic}
          onClick={() => {
            editor.chain().focus().toggleItalic().run();
          }}
        />
        <IconButton
          label={t("composer.format.link")}
          icon="link"
          aria-pressed={marks.link}
          onClick={openLink}
        />
      </div>
      {link === undefined ? null : (
        <LinkForm
          key={`${String(link.from)}-${String(link.to)}`}
          initialWords={link.words}
          initialAddress={link.href}
          onApply={(words, href) => {
            editor
              .chain()
              .focus()
              .insertContentAt(
                { from: link.from, to: link.to },
                { type: "text", text: words, marks: [{ type: "link", attrs: { href } }] },
              )
              .run();
            setLink(undefined);
          }}
          onRemove={
            link.href === ""
              ? undefined
              : () => {
                  editor
                    .chain()
                    .focus()
                    .setTextSelection({ from: link.from, to: link.to })
                    .unsetLink()
                    .run();
                  setLink(undefined);
                }
          }
          onCancel={() => {
            setLink(undefined);
            editor.commands.focus();
          }}
        />
      )}
      <EditorContent editor={editor} className={styles["text"]} />
    </div>
  );
}
