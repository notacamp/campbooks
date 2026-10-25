import { Extension } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import { Suggestion, type SuggestionProps } from "@tiptap/suggestion";

/** Where the editor tells the page that a menu opened, changed or closed. */
export interface MenuBridge<TItem> {
  /** The menu is open, or its query changed. */
  show: (props: SuggestionProps<TItem, TItem>) => void;
  /** The menu closed. */
  hide: () => void;
  /** A key was pressed while it is open: `true` when the menu used it. */
  key: (event: KeyboardEvent) => boolean;
}

export const SLASH_KEY = new PluginKey("pagesSlashMenu");
export const MENTION_KEY = new PluginKey("pagesMentionMenu");

/**
 * The menu `/` opens at the start of a line or after a space: blocks to
 * insert. `items` filters by what was typed after the slash.
 */
export function createSlashMenu<TItem>(
  bridge: MenuBridge<TItem>,
  items: (query: string) => TItem[],
  run: (item: TItem, range: { from: number; to: number }) => void,
) {
  return Extension.create({
    name: "pagesSlashMenu",
    addProseMirrorPlugins() {
      return [
        Suggestion<TItem, TItem>({
          editor: this.editor,
          pluginKey: SLASH_KEY,
          char: "/",
          items: ({ query }) => items(query),
          command: ({ range, props }) => {
            run(props, range);
          },
          render: () => ({
            onStart: (props) => {
              bridge.show(props);
            },
            onUpdate: (props) => {
              bridge.show(props);
            },
            onExit: () => {
              bridge.hide();
            },
            onKeyDown: ({ event }) => bridge.key(event),
          }),
        }),
      ];
    },
  });
}

/**
 * The menu `@` opens: people, threads, tasks, events and pages found by name
 * as the member types. Spaces are part of the query (a name has them).
 */
export function createMentionMenu<TItem>(
  bridge: MenuBridge<TItem>,
  search: (query: string, signal: AbortSignal) => Promise<TItem[]>,
  run: (item: TItem, range: { from: number; to: number }) => void,
) {
  return Extension.create({
    name: "pagesMentionMenu",
    addProseMirrorPlugins() {
      return [
        Suggestion<TItem, TItem>({
          editor: this.editor,
          pluginKey: MENTION_KEY,
          char: "@",
          allowSpaces: true,
          debounce: 150,
          items: ({ query, signal }) => search(query, signal),
          command: ({ range, props }) => {
            run(props, range);
          },
          render: () => ({
            onStart: (props) => {
              bridge.show(props);
            },
            onUpdate: (props) => {
              bridge.show(props);
            },
            onExit: () => {
              bridge.hide();
            },
            onKeyDown: ({ event }) => bridge.key(event),
          }),
        }),
      ];
    },
  });
}
