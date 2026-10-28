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
 */
export function composerExtensions(): Extensions {
  const send = () => {
    // Replaced by the composer once it has the editor (see `onSend` there).
  };
  // ⌘↵ would split the block here (TipTap binds it): it sends instead.
  const Send = Extension.create({
    name: "composerSend",
    priority: 1000,
    addStorage() {
      return { send };
    },
    addKeyboardShortcuts() {
      return {
        "Mod-Enter": () => {
          this.storage.send();
          return true;
        },
      };
    },
  });
  return [
    Send,
    ComposerKit.configure({
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
