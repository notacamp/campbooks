import { act } from "@testing-library/react";
import type { Editor } from "@tiptap/core";

/** The editor behind a composer's text box (TipTap leaves it on the element). */
export function editorOf(box: HTMLElement): Editor {
  return (box as HTMLElement & { editor: Editor }).editor;
}

/** Type `text` at the cursor of a composer's text box, as the member would. */
export function typeInto(box: HTMLElement, text: string): void {
  act(() => {
    editorOf(box).chain().focus().insertContent(text).run();
  });
}

/** Replace everything in a composer's text box with `text`. */
export function replaceText(box: HTMLElement, text: string): void {
  act(() => {
    editorOf(box).chain().focus().selectAll().insertContent(text).run();
  });
}
