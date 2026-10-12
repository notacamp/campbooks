import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import type { Signal } from "../../tokens/tokens";

import styles from "./Pill.module.css";

export interface PillProps {
  /** What the item asks of you. The colour carries the meaning. */
  signal: Signal;
  /** Defaults to the signal's name (Reply, Decide, ...). */
  children?: ReactNode | undefined;
}

/** A signal label: a soft fill and a dot in the signal's colour. */
export function Pill({ signal, children }: PillProps) {
  const { t } = useTranslation("ui");
  return (
    <span className={styles["pill"]} data-signal={signal}>
      {children ?? t(`signal.${signal}`)}
    </span>
  );
}
