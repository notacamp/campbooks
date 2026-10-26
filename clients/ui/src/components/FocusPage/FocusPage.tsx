import type { ReactNode, SubmitEventHandler } from "react";

import type { Accent } from "../../tokens/tokens";
import { Heading } from "../Typography/Typography";
import { Visor } from "../Visor/Visor";

import styles from "./FocusPage.module.css";

/** The product mark: the visor and the name, for pages outside the app frame. */
export function Brand({ name }: { name: string }) {
  return (
    <span className={styles["brand"]}>
      <Visor size="md" />
      <span className={styles["brandName"]}>{name}</span>
    </span>
  );
}

export interface FocusPageProps {
  /** One sentence: the question or the state ("What should we call it?"). */
  title: string;
  /** Above the title: a visor, a workspace mark. */
  leading?: ReactNode | undefined;
  /** Small caps line above the title ("Step 1 of 2"). */
  eyebrow?: string | undefined;
  /** One or two sentences under the title. */
  lead?: ReactNode | undefined;
  /** The answer: a field, choices, a message. */
  children?: ReactNode | undefined;
  /** The buttons; the first is usually the primary one. */
  actions?: ReactNode | undefined;
  /** The bar above the page: the brand, a way out. */
  top?: ReactNode | undefined;
  /** Quiet text at the bottom (where to get help). */
  footer?: ReactNode | undefined;
  /** Preview an accent on this page only. */
  accent?: Accent | undefined;
  /** Makes the column a form: Enter submits. */
  onSubmit?: SubmitEventHandler<HTMLFormElement> | undefined;
  /** Accessible name of the form (defaults to the title). */
  formLabel?: string | undefined;
}

/**
 * A page that asks one thing, centred and calm: signing in, each step of
 * creating a workspace, accepting an invitation. Stacks to a phone width.
 */
export function FocusPage({
  title,
  leading,
  eyebrow,
  lead,
  children,
  actions,
  top,
  footer,
  accent,
  onSubmit,
  formLabel,
}: FocusPageProps) {
  const content = (
    <>
      {leading === undefined ? null : <div className={styles["leading"]}>{leading}</div>}
      <div className={styles["heading"]}>
        {eyebrow === undefined ? null : <p className={styles["eyebrow"]}>{eyebrow}</p>}
        <Heading level={1} size="page">
          {title}
        </Heading>
        {lead === undefined ? null : <div className={styles["lead"]}>{lead}</div>}
      </div>
      {children === undefined ? null : <div className={styles["body"]}>{children}</div>}
      {actions === undefined ? null : <div className={styles["actions"]}>{actions}</div>}
    </>
  );
  return (
    <div className={styles["page"]} data-accent={accent}>
      {top === undefined ? null : <div className={styles["top"]}>{top}</div>}
      <main className={styles["main"]}>
        {onSubmit === undefined ? (
          <div className={styles["column"]}>{content}</div>
        ) : (
          <form
            className={styles["column"]}
            aria-label={formLabel ?? title}
            noValidate
            onSubmit={(event) => {
              event.preventDefault();
              onSubmit(event);
            }}
          >
            {content}
          </form>
        )}
      </main>
      {footer === undefined ? null : <footer className={styles["footer"]}>{footer}</footer>}
    </div>
  );
}
