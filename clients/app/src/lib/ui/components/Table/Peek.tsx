import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { Button, IconButton, useFocusTrap } from "@notacamp/ui";

import styles from "./Peek.module.css";

export interface RowPeekProps {
  /** Names the panel ("Row page"). */
  label: string;
  /** Where the row is ("Clients"), over its title. */
  crumb: ReactNode;
  /** The "Open as page" button's text. */
  openAsPage?: string | undefined;
  onOpenAsPage?: (() => void) | undefined;
  closeLabel: string;
  onClose: () => void;
  /** Move the row to the trash (not for a reader): the label names it. */
  trashLabel?: string | undefined;
  onTrash?: (() => void) | undefined;
  children: ReactNode;
}

/**
 * A row opened beside its table (in the page's top layer, so nothing clips it): its title, its properties and its page.
 * From the large width it docks to the right edge over the table; on a phone
 * it fills the screen. Focus stays inside it; Escape (or Close) closes it and
 * puts focus back where it was.
 */
export function RowPeek({
  label,
  crumb,
  openAsPage,
  onOpenAsPage,
  closeLabel,
  onClose,
  trashLabel,
  onTrash,
  children,
}: RowPeekProps) {
  const panel = useRef<HTMLElement>(null);
  useFocusTrap(panel, true);
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useEffect(() => {
    const element = panel.current;
    const onKeyDown = (event: KeyboardEvent) => {
      // A menu or picker inside closes itself on Escape first.
      if (event.key === "Escape" && !event.defaultPrevented) {
        event.stopPropagation();
        event.preventDefault();
        close.current();
      }
    };
    element?.addEventListener("keydown", onKeyDown);
    return () => {
      element?.removeEventListener("keydown", onKeyDown);
    };
  }, []);
  return createPortal(
    <aside
      ref={panel}
      role="dialog"
      aria-modal="true"
      aria-label={label}
      tabIndex={-1}
      className={styles["peek"]}
    >
      <div className={styles["bar"]}>
        <span className={styles["crumb"]}>{crumb}</span>
        {openAsPage === undefined || onOpenAsPage === undefined ? null : (
          <Button variant="ghost" onClick={onOpenAsPage}>
            {openAsPage}
          </Button>
        )}
        {onTrash === undefined || trashLabel === undefined ? null : (
          <IconButton icon="trash" label={trashLabel} onClick={onTrash} />
        )}
        <IconButton icon="close" label={closeLabel} onClick={onClose} />
      </div>
      <div className={styles["body"]}>{children}</div>
    </aside>,
    document.body,
  );
}

export interface PropertyLineProps {
  /** The property's name. */
  name: string;
  /** Its mark (the type's glyph). */
  glyph?: ReactNode | undefined;
  children: ReactNode;
}

/** One property of a row in the peek or the row page: its name, then its value. */
export function PropertyLine({ name, glyph, children }: PropertyLineProps) {
  return (
    <div className={styles["line"]} role="group" aria-label={name}>
      <span className={styles["name"]}>
        {glyph}
        <span className={styles["nameText"]}>{name}</span>
      </span>
      <div className={styles["value"]}>{children}</div>
    </div>
  );
}

/** The properties of a row, one under another. */
export function PropertyList({ children }: { children: ReactNode }) {
  return <div className={styles["properties"]}>{children}</div>;
}
