import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { useFocusTrap } from "./use-focus-trap";

import styles from "./Overlay.module.css";

export interface OverlayProps {
  open: boolean;
  onClose: () => void;
  /** Accessible name of the dialog. */
  label: string;
  children: ReactNode;
}

/**
 * A modal dialog over a scrim: traps focus, closes on Escape or a click on
 * the scrim, and returns focus where it was. Rendered into <body>.
 */
export function Overlay({ open, onClose, label, children }: OverlayProps) {
  const scrim = useRef<HTMLDivElement>(null);
  const dialog = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });
  useFocusTrap(dialog, open);

  useEffect(() => {
    const scrimElement = scrim.current;
    const dialogElement = dialog.current;
    if (!open || scrimElement === null || dialogElement === null) {
      return;
    }
    const onScrimDown = (event: MouseEvent) => {
      if (event.target === scrimElement) {
        close.current();
      }
    };
    // Escape closes the dialog, and only the dialog: it stops here so the
    // global shortcut listener never sees it.
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        event.preventDefault();
        close.current();
      }
    };
    scrimElement.addEventListener("mousedown", onScrimDown);
    dialogElement.addEventListener("keydown", onKeyDown);
    return () => {
      scrimElement.removeEventListener("mousedown", onScrimDown);
      dialogElement.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  if (!open) {
    return null;
  }
  return createPortal(
    <div ref={scrim} className={styles["scrim"]}>
      <div
        ref={dialog}
        className={styles["dialog"]}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}
