import type { ReactNode } from "react";

import { cx } from "../../internal/cx";

import styles from "./Typography.module.css";

export type HeadingSize = "hero" | "page" | "title" | "pane" | "section";

export interface HeadingProps {
  /** Document outline level (h1–h4), independent of the visual size. */
  level: 1 | 2 | 3 | 4;
  /** hero: the home sentence · page: a reader title · title: empty states ·
   *  pane: a list pane heading · section: small group headings. */
  size?: HeadingSize | undefined;
  id?: string | undefined;
  className?: string | undefined;
  children: ReactNode;
}

export function Heading({ level, size = "page", id, className, children }: HeadingProps) {
  const Tag = `h${String(level)}` as "h1" | "h2" | "h3" | "h4";
  return (
    <Tag id={id} className={cx(styles["heading"], className)} data-size={size}>
      {children}
    </Tag>
  );
}

export type TextTone = "primary" | "secondary" | "tertiary" | "danger" | "accent";

export interface TextProps {
  as?: "p" | "span" | "div" | "dt" | "dd" | "time" | "small" | undefined;
  tone?: TextTone | undefined;
  size?: "xs" | "sm" | "md" | "lg" | undefined;
  weight?: "regular" | "medium" | "semibold" | undefined;
  /** Tabular figures, for times and amounts that line up. */
  numeric?: boolean | undefined;
  /** Small caps label (the home date line). */
  caps?: boolean | undefined;
  /** Cut to one line with an ellipsis. */
  truncate?: boolean | undefined;
  id?: string | undefined;
  dateTime?: string | undefined;
  className?: string | undefined;
  children: ReactNode;
}

/** Body text in one of the semantic tones. */
export function Text({
  as: Tag = "p",
  tone = "primary",
  size = "md",
  weight = "regular",
  numeric = false,
  caps = false,
  truncate = false,
  className,
  children,
  ...rest
}: TextProps) {
  return (
    <Tag
      {...rest}
      className={cx(styles["text"], className)}
      data-tone={tone}
      data-size={size}
      data-weight={weight}
      data-numeric={numeric || undefined}
      data-caps={caps || undefined}
      data-truncate={truncate || undefined}
    >
      {children}
    </Tag>
  );
}

/** Text for screen readers only. */
export function VisuallyHidden({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <span id={id} className={styles["visuallyHidden"]}>
      {children}
    </span>
  );
}
