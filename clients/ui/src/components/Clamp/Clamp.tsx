import { useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";

import { Button } from "../Button/Button";

import styles from "./Clamp.module.css";

export interface ClampProps {
  /** Long content: an email, a document. */
  children: ReactNode;
  /** The button that shows it all ("Show the whole email"). */
  moreLabel: string;
  /** The button that folds it back ("Show less"). */
  lessLabel: string;
}

/**
 * Long content shown a few lines deep (`--clamp-height`) with a fade, and a
 * button to read it all in place. The button shows only when there is more
 * to see; content that grows later (a message frame measuring itself) is
 * watched. Focus moving into the hidden part opens it, so the keyboard never
 * lands somewhere out of sight.
 */
export function Clamp({ children, moreLabel, lessLabel }: ClampProps) {
  const id = useId();
  const box = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  // Without a way to measure, offer the button rather than hide content.
  const [overflows, setOverflows] = useState(() => typeof ResizeObserver === "undefined");

  useLayoutEffect(() => {
    const outer = box.current;
    const inner = content.current;
    if (outer === null || inner === null || typeof ResizeObserver === "undefined") {
      return;
    }
    const check = () => {
      // Only a clamped box can tell whether there is more.
      if (outer.dataset["open"] === undefined) {
        setOverflows(inner.scrollHeight > outer.clientHeight + 1);
      }
    };
    const observer = new ResizeObserver(check);
    observer.observe(inner);
    observer.observe(outer);
    check();
    // Focus landing in the clamped box (Tab into a link far down, a click
    // into the frame) opens it, so it never sits out of sight.
    const reveal = () => {
      if (outer.dataset["open"] === undefined && inner.scrollHeight > outer.clientHeight + 1) {
        setOpen(true);
      }
    };
    outer.addEventListener("focusin", reveal);
    return () => {
      observer.disconnect();
      outer.removeEventListener("focusin", reveal);
    };
  }, []);

  return (
    <div className={styles["clamp"]}>
      <div
        ref={box}
        id={id}
        className={styles["box"]}
        data-open={open ? "" : undefined}
        data-fade={!open && overflows ? "" : undefined}
      >
        <div ref={content}>{children}</div>
      </div>
      {overflows || open ? (
        <Button
          variant="link"
          aria-expanded={open}
          aria-controls={id}
          onClick={() => {
            setOpen(!open);
          }}
        >
          {open ? lessLabel : moreLabel}
        </Button>
      ) : null}
    </div>
  );
}
