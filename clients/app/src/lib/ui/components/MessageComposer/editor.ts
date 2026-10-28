import { Extension, type AnyExtension, type Extensions } from "@tiptap/core";
import { StarterKit } from "@tiptap/starter-kit";

import { isLinkable } from "./marks";

/**
 * The extensions whose own key bindings are turned off: the shortcut
 * registry handles those keys (`compose.*`), so each key has one handler.
 * A key TipTap handled would be `preventDefault`ed and never reach it.
 */
const REGISTRY_KEYS = new Set([
  "bold",
  "italic",
  "strike",
  "bulletList",
  "orderedList",
  "blockquote",
]);

/**
 * StarterKit's pieces, narrowed to the composer's schema: no keys of their
 * own where the registry has them, a link that keeps only its address, and
 * list items of one paragraph (the marks hold one level of lists, a line each).
 */
const ComposerKit = StarterKit.extend({
  addExtensions() {
    const parent = (this.parent?.() ?? []) as AnyExtension[];
    return parent.map((extension) => {
      if (REGISTRY_KEYS.has(extension.name)) {
        return extension.extend({
          addKeyboardShortcuts() {
            return {};
          },
        });
      }
      if (extension.name === "link") {
        return extension.extend({
          addAttributes() {
            return { href: { default: null } };
          },
        });
      }
      if (extension.name === "listItem") {
        return extension.extend({ content: "paragraph" });
      }
      return extension;
    });
  },
});

/**
 * The composer's fixed schema (paragraphs, line breaks, bold, italic,
 * strikethrough, links, bulleted and numbered lists, quotes, undo and redo),
 * not the Pages set. Anything pasted that the schema lacks is dropped.
 * `compact` is a signature's: no lists, quotes or strikethrough.
 */
export function composerExtensions(options: { compact?: boolean } = {}): Extensions {
  const send = () => {
    // Replaced by the composer once it has the editor (see `onSend` there).
  };
  const escape = () => {
    // Replaced by the composer, as `send` is.
  };
  // ⌘↵ would split the block here (TipTap binds it): it sends instead. Esc,
  // ⌘B and ⌘I never reach the shortcut registry from the text (ProseMirror
  // prevents their default), so the editor handles them itself: Esc goes to
  // the composer, ⌘B and ⌘I format, as the registry's keys do elsewhere.
  const Send = Extension.create({
    name: "composerSend",
    priority: 1000,
    addStorage() {
      return { send, escape };
    },
    addKeyboardShortcuts() {
      return {
        "Mod-Enter": () => {
          this.storage.send();
          return true;
        },
        Escape: () => {
          this.storage.escape();
          return true;
        },
        "Mod-b": () => this.editor.commands.toggleBold(),
        "Mod-i": () => this.editor.commands.toggleItalic(),
      };
    },
  });
  return [
    Send,
    ComposerKit.configure({
      // A signature: bold, italic and links, in lines; nothing a list or a
      // quote would add.
      ...(options.compact === true
        ? {
            bulletList: false,
            orderedList: false,
            listItem: false,
            blockquote: false,
            strike: false,
          }
        : {}),
      heading: false,
      code: false,
      codeBlock: false,
      horizontalRule: false,
      underline: false,
      dropcursor: false,
      trailingNode: false,
      link: {
        openOnClick: false,
        autolink: true,
        linkOnPaste: true,
        isAllowedUri: (url) => isLinkable(url),
      },
    }),
  ];
}
