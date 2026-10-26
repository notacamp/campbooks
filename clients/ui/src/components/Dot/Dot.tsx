import type { Signal } from "../../tokens/tokens";

import styles from "./Dot.module.css";

export interface DotProps {
  /** Signal colour; neutral when omitted. */
  signal?: Signal | undefined;
}

/** A small decorative status dot. Always pair it with text. */
export function Dot({ signal }: DotProps) {
  return <span className={styles["dot"]} data-signal={signal} aria-hidden="true" />;
}
