import { useEffect, useId, useRef, useState, type ReactNode, type RefObject } from "react";

import styles from "./Popover.module.css";

export interface PopoverTriggerProps {
  ref: RefObject<HTMLButtonElement | null>;
  "aria-expanded": boolean;
  "aria-controls": string;
  "aria-haspopup": "dialog";
  onClick: () => void;
}

export interface PopoverProps {
  /** Accessible name of the panel ("Account"). */
  label: string;
  /** Renders the button that opens it; spread the props onto a <button>. */
  renderTrigger: (props: PopoverTriggerProps) => ReactNode;
  /** The panel's content; a function gets `close`. */
  children: ReactNode | ((close: () => void) => ReactNode);
  /** Where the panel opens relative to the trigger. */
  placement?: "top-start" | "bottom-start" | "bottom-end" | undefined;
  /** Controlled open state (optional). */
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
}

const FOCUSABLE = "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled])";

/**
 * A small non-modal panel anchored to its trigger (the account menu). Closes
 * on Escape (focus back on the trigger) and on a click outside; Tab moves
 * through it and out as usual.
 */
export function Popover({
  label,
  renderTrigger,
  children,
  placement = "bottom-end",
  open: controlledOpen,
  onOpenChange,
}: PopoverProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const open = controlledOpen ?? uncontrolledOpen;
  const panelId = useId();
  const anchor = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const setOpen = (next: boolean) => {
    setUncontrolledOpen(next);
    onOpenChange?.(next);
  };
  // The listeners below always call the latest setter.
  const change = useRef(setOpen);
  useEffect(() => {
    change.current = setOpen;
  });

  useEffect(() => {
    if (!open) {
      return;
    }
    // A control marked `data-autofocus` (a date grid's chosen day) takes the
    // focus; otherwise the first one.
    (
      panel.current?.querySelector<HTMLElement>("[data-autofocus]") ??
      panel.current?.querySelector<HTMLElement>(FOCUSABLE)
    )?.focus();
    const onPointerDown = (event: PointerEvent) => {
      if (event.target instanceof Node && anchor.current?.contains(event.target) !== true) {
        change.current(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        event.preventDefault();
        change.current(false);
        triggerRef.current?.focus();
      }
    };
    const anchorElement = anchor.current;
    document.addEventListener("pointerdown", onPointerDown);
    anchorElement?.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      anchorElement?.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  const close = () => {
    setOpen(false);
  };

  return (
    <div ref={anchor} className={styles["anchor"]}>
      {renderTrigger({
        ref: triggerRef,
        "aria-expanded": open,
        "aria-controls": panelId,
        "aria-haspopup": "dialog",
        onClick: () => {
          setOpen(!open);
        },
      })}
      <div
        ref={panel}
        id={panelId}
        className={styles["panel"]}
        role="dialog"
        aria-label={label}
        data-placement={placement}
        hidden={!open}
      >
        {open ? (typeof children === "function" ? children(close) : children) : null}
      </div>
    </div>
  );
}

/** A titled group of settings inside a popover (Theme, Density). */
export function PopoverSection({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <div className={styles["section"]}>
      {title === undefined ? null : <p className={styles["sectionTitle"]}>{title}</p>}
      {children}
    </div>
  );
}
