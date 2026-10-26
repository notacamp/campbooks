import { textblockTypeInputRule, type Extension } from "@tiptap/core";
import { Collaboration } from "@tiptap/extension-collaboration";
import { CodeBlock } from "@tiptap/extension-code-block";
import { Link } from "@tiptap/extension-link";
import { Placeholder } from "@tiptap/extension-placeholder";
import { TaskItem } from "@tiptap/extension-task-item";
import { TaskList } from "@tiptap/extension-task-list";
import { StarterKit } from "@tiptap/starter-kit";
import type * as Y from "yjs";

import { Bid } from "./bid";
import { codeLanguage } from "./languages";
import {
  Callout,
  Conflict,
  Embed,
  External,
  FileBlock,
  ImageBlock,
  Mention,
  PageLink,
  Toggle,
} from "./nodes";

/** Links may only go to the web and to mail: the server refuses the rest. */
export const isAllowedLink = (url: string) => /^(https?:|mailto:)/iu.test(url.trim());

/** A fenced block (three backticks and a language) names only a language the server knows. */
const FENCE = /^```([\w#+-]*)[\s\u00A0]$/u;

const SafeCodeBlock = CodeBlock.extend({
  addAttributes() {
    return {
      language: {
        default: null,
        parseHTML: (element) =>
          codeLanguage(
            element.firstElementChild?.getAttribute("class")?.replace("language-", "") ??
              element.dataset["language"],
          ),
        renderHTML: (attributes) => {
          const language: unknown = attributes["language"];
          return typeof language === "string" ? { "data-language": language } : {};
        },
      },
    };
  },
  addInputRules() {
    return [
      textblockTypeInputRule({
        find: FENCE,
        type: this.type,
        getAttributes: (match) => ({ language: codeLanguage(match[1]) }),
      }),
    ];
  },
});

/** A link keeps its address and nothing else: no target, no rel, no class. */
const SafeLink = Link.extend({
  addAttributes() {
    return { href: { default: null } };
  },
});

export interface EditorExtensionOptions {
  /** The page's document. */
  doc: Y.Doc;
  /** The line shown on an empty paragraph, and on an empty heading. */
  placeholders: { paragraph: string; heading: string };
  /** Extensions that open the `/` and `@` menus, made by the editor component. */
  menus: readonly Extension[];
}

/**
 * The schema of a page: the block names and attributes the server's allowlist
 * knows (`crates/modules/pages/src/domain/document.rs`), the document held in
 * Yjs, and no mark or node besides.
 */
export function createExtensions({ doc, placeholders, menus }: EditorExtensionOptions) {
  return [
    StarterKit.configure({
      // Yjs keeps the undo history (⌘Z, ⇧⌘Z) and the schema has no underline.
      undoRedo: false,
      underline: false,
      link: false,
      codeBlock: false,
      heading: { levels: [1, 2, 3] },
    }),
    SafeCodeBlock,
    SafeLink.configure({
      openOnClick: false,
      autolink: true,
      isAllowedUri: (url) => isAllowedLink(url),
    }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Callout,
    Toggle,
    Mention,
    Embed,
    PageLink,
    FileBlock,
    ImageBlock,
    External,
    Conflict,
    Bid,
    Placeholder.configure({
      placeholder: ({ node }) =>
        node.type.name === "heading" ? placeholders.heading : placeholders.paragraph,
    }),
    Collaboration.configure({ document: doc, field: "default" }),
    ...menus,
  ];
}
