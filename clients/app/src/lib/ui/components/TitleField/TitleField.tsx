import {
  useCallback,
  useLayoutEffect,
  useRef,
  type ChangeEvent,
  type ComponentPropsWithRef,
  type KeyboardEvent,
} from "react";

import { cx } from "../../internal/cx";

import styles from "./TitleField.module.css";

export interface TitleFieldProps extends Omit<
  ComponentPropsWithRef<"textarea">,
  "rows" | "cols" | "wrap"
> {
  /** Accessible name ("Page title"); the field has no visible label. */
  label: string;
}

/**
 * A page's title, edited in place: a big heading that is a text box. It is
 * one paragraph that wraps onto as many lines as it needs (never cut off),
 * grows with what is typed, and takes no line breaks: Enter does nothing here
 * and a pasted break becomes a space.
 */
export function TitleField({
  label,
  className,
  ref,
  onChange,
  onKeyDown,
  ...rest
}: TitleFieldProps) {
  const field = useRef<HTMLTextAreaElement | null>(null);
  const setRef = useCallback(
    (node: HTMLTextAreaElement | null) => {
      field.current = node;
      if (typeof ref === "function") {
        ref(node);
      } else if (ref !== undefined && ref !== null) {
        ref.current = node;
      }
    },
    [ref],
  );
  // Fit the height to the text, now and when the width (so the wrapping) changes.
  useLayoutEffect(() => {
    const fit = () => {
      const node = field.current;
      if (node !== null) {
        node.style.setProperty("--title-height", "auto");
        node.style.setProperty("--title-height", `${String(node.scrollHeight)}px`);
      }
    };
    fit();
    window.addEventListener("resize", fit);
    return () => {
      window.removeEventListener("resize", fit);
    };
  });
  const change = (event: ChangeEvent<HTMLTextAreaElement>) => {
    if (/[\r\n]/u.test(event.target.value)) {
      event.target.value = event.target.value.replaceAll(/\s*[\r\n]+\s*/gu, " ");
    }
    onChange?.(event);
  };
  const keys = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
    }
    onKeyDown?.(event);
  };
  return (
    <textarea
      {...rest}
      ref={setRef}
      rows={1}
      className={cx(styles["title"], className)}
      aria-label={label}
      autoComplete="off"
      spellCheck
      onChange={change}
      onKeyDown={keys}
    />
  );
}
