import { useId, type ReactNode } from "react";

import { Visor, type VisorState } from "@notacamp/ui";

import styles from "./Offer.module.css";

export interface OfferProps {
  /** The question or the news, in one sentence. */
  title: ReactNode;
  /** A plain line under it. */
  detail?: ReactNode | undefined;
  /** The assistant's face: reading while it asks, thinking while it works. */
  visor?: VisorState | undefined;
  /** Choices and a preview, between the question and the actions. */
  children?: ReactNode | undefined;
  /** Buttons, each with its shortcut. */
  actions: ReactNode;
  /** A last quiet line ("You can undo it"). */
  foot?: ReactNode | undefined;
}

/**
 * Something the assistant offers to do next, docked over the screen without
 * blocking it (a non-modal dialog): above the toasts on the right on wide
 * screens, across the bottom on phones. The list stays usable behind it.
 */
export function Offer({ title, detail, visor = "reading", children, actions, foot }: OfferProps) {
  const titleId = useId();
  return (
    <section className={styles["offer"]} role="dialog" aria-modal="false" aria-labelledby={titleId}>
      <div className={styles["head"]}>
        <Visor state={visor} size="md" />
        <div className={styles["text"]}>
          <p id={titleId} className={styles["title"]}>
            {title}
          </p>
          {detail === undefined ? null : <p className={styles["detail"]}>{detail}</p>}
        </div>
      </div>
      {children}
      <div className={styles["actions"]}>{actions}</div>
      {foot === undefined ? null : <p className={styles["foot"]}>{foot}</p>}
    </section>
  );
}
