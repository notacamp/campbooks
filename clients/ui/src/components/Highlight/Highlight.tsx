import { Fragment } from "react";

import styles from "./Highlight.module.css";

/** A run of text and whether it matched the search (the API's `Segment`). */
export interface HighlightSegment {
  text: string;
  hit: boolean;
}

export interface HighlightProps {
  segments: readonly HighlightSegment[];
}

/**
 * Text with the words that matched a search marked. The segments come from
 * the server and are rendered as text only (never as HTML), so a subject
 * that looks like markup stays a subject.
 */
export function Highlight({ segments }: HighlightProps) {
  return (
    <>
      {segments.map((segment, index) =>
        segment.hit ? (
          <mark key={index} className={styles["mark"]}>
            {segment.text}
          </mark>
        ) : (
          <Fragment key={index}>{segment.text}</Fragment>
        ),
      )}
    </>
  );
}
