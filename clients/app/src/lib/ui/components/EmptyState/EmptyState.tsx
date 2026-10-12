import type { ReactNode } from "react";

import { Heading } from "../Typography/Typography";
import { Visor, type VisorState } from "../Visor/Visor";

import styles from "./EmptyState.module.css";

export interface EmptyStateProps {
  title: string;
  children?: ReactNode | undefined;
  /** happy when there is nothing to do; thinking when something failed and is retryable. */
  visor?: VisorState | undefined;
  visorSize?: "lg" | "xl" | undefined;
  action?: ReactNode | undefined;
  headingLevel?: 2 | 3 | undefined;
}

/** What a place says when it has nothing to show, or could not load. */
export function EmptyState({
  title,
  children,
  visor = "happy",
  visorSize = "xl",
  action,
  headingLevel = 2,
}: EmptyStateProps) {
  return (
    <div className={styles["empty"]}>
      <Visor state={visor} size={visorSize} />
      <Heading level={headingLevel} size="title">
        {title}
      </Heading>
      {children === undefined ? null : <p className={styles["body"]}>{children}</p>}
      {action}
    </div>
  );
}
