import { useTranslation } from "react-i18next";

import { cx } from "../../internal/cx";

import styles from "./Visor.module.css";

export const VISOR_STATES = [
  "idle",
  "reading",
  "thinking",
  "unsure",
  "happy",
  "alert",
  "asleep",
] as const;
export type VisorState = (typeof VISOR_STATES)[number];

export interface VisorProps {
  /** What the assistant is doing. Its eyes say it; motion stops under reduced motion. */
  state?: VisorState | undefined;
  /** sm 22px · md ×1.4 · lg ×2 · xl ×2.8 */
  size?: "sm" | "md" | "lg" | "xl" | undefined;
  /** Announce the state ("Campbooks is reading"). Decorative by default. */
  announce?: boolean | undefined;
  className?: string | undefined;
}

/** The assistant's face: a dark glass visor with two eyes. */
export function Visor({ state = "idle", size = "sm", announce = false, className }: VisorProps) {
  const { t } = useTranslation("ui");
  return (
    <span
      className={cx(styles["visor"], className)}
      data-state={state}
      data-size={size}
      {...(announce ? { role: "img", "aria-label": t(`visor.${state}`) } : { "aria-hidden": true })}
    >
      <i className={styles["eye"]} />
      <i className={styles["eye"]} />
    </span>
  );
}
