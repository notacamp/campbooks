import { cx } from "../../internal/cx";
import { Icon } from "../Icon/Icon";
import { Visor } from "../Visor/Visor";

import styles from "./ThinkingTrace.module.css";

export interface ThinkingTraceProps {
  /** What Scout is doing while the trace grows: "Scout is thinking…". */
  label: string;
  /** The thinking so far: model text, shown as plain text only. */
  text: string;
  /** Still arriving: shown open, following the newest line. Once false, folded away. */
  live: boolean;
  /** The folded trace's summary: "How Scout thought about it". */
  summary: string;
}

/**
 * Scout's thinking as it arrives: a few muted lines that follow the newest,
 * under a status line that says what is happening. Once the answer is
 * written the trace folds into a native disclosure, one click from the
 * thinking that led to it. Only the status is announced; the text itself is
 * not read aloud as it grows.
 */
export function ThinkingTrace({ label, text, live, summary }: ThinkingTraceProps) {
  if (live) {
    return (
      <div className={styles["trace"]}>
        <p className={styles["status"]} role="status">
          <Visor state="thinking" size="xs" />
          {label}
        </p>
        {text === "" ? null : (
          // The newest lines show; older ones slide out of view at the top.
          // Nothing scrolls, so there is nothing to reach by keyboard.
          <div className={cx(styles["lines"], styles["tail"])} aria-live="off">
            {text}
          </div>
        )}
      </div>
    );
  }
  if (text === "") {
    return null;
  }
  return (
    <details className={styles["trace"]}>
      <summary className={styles["summary"]}>
        <Icon name="chevron-down" className={styles["chevron"]} />
        <span>{summary}</span>
      </summary>
      <div className={styles["lines"]}>{text}</div>
    </details>
  );
}
