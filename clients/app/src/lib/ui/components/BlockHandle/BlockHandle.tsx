import type { CSSProperties, DragEventHandler, ReactNode } from "react";

import { IconButton } from "@notacamp/ui";

import styles from "./BlockHandle.module.css";

export interface BlockHandleProps {
  /** Where the block's first line is, in the editor's coordinates. */
  top: number;
  addLabel: string;
  onAdd: () => void;
  menuLabel: string;
  /** Click the grip for the block menu; drag it to move the block. */
  onMenu: () => void;
  onDragStart: DragEventHandler<HTMLButtonElement>;
  onDragEnd?: DragEventHandler<HTMLButtonElement> | undefined;
  /** The block menu, when open (it opens under the grip). */
  menu?: ReactNode | undefined;
}

/**
 * The two small controls at the left of the block the pointer is on: add a
 * block below, and a grip to drag the block or open its menu. Hidden on
 * phones, where the formatting bar and the long-press menu do the job.
 */
export function BlockHandle({
  top,
  addLabel,
  onAdd,
  menuLabel,
  onMenu,
  onDragStart,
  onDragEnd,
  menu,
}: BlockHandleProps) {
  return (
    <div className={styles["handle"]} style={{ "--top": `${String(top)}px` } as CSSProperties}>
      <IconButton icon="plus" label={addLabel} onClick={onAdd} />
      <IconButton
        icon="more"
        label={menuLabel}
        draggable
        onClick={onMenu}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
      />
      {menu}
    </div>
  );
}
