import type { ReactNode } from "react";

import { Skeleton } from "@notacamp/ui";

import styles from "./RulePreview.module.css";

export interface RulePreviewSample {
  /** The conversation's subject. Untrusted mail text: shown as plain text only. */
  subject: string;
  /** Who sent it. */
  sender: string;
  /** When, already formatted ("12 Oct"). */
  when: string;
}

export interface RulePreviewProps {
  /** The rule, as one sentence ("Mail from Rita Gomes always stays in your inbox"). */
  sentence: ReactNode;
  /** What it would match, in words ("41 emails in ines@example.com"); omit while unknown. */
  summary?: ReactNode | undefined;
  /** A few of them, newest first. */
  samples?: readonly RulePreviewSample[] | undefined;
  /** Name of the samples list, for screen readers ("Emails it matches"). */
  samplesLabel?: string | undefined;
  /** The summary is being counted. */
  pending?: boolean | undefined;
  /** A short tag before the sentence ("Rule"). */
  tag?: ReactNode | undefined;
}

/**
 * A rule shown before it is saved: the sentence it will read as, how much
 * mail it matches, and a few of those emails.
 */
export function RulePreview({
  sentence,
  summary,
  samples = [],
  samplesLabel,
  pending = false,
  tag,
}: RulePreviewProps) {
  return (
    <div className={styles["preview"]}>
      <p className={styles["sentence"]}>
        {tag === undefined ? null : <span className={styles["tag"]}>{tag}</span>}
        <span>{sentence}</span>
      </p>
      {pending ? (
        <Skeleton width="60%" />
      ) : summary === undefined ? null : (
        <p className={styles["summary"]}>{summary}</p>
      )}
      {samples.length === 0 ? null : (
        <ul className={styles["samples"]} aria-label={samplesLabel}>
          {samples.map((sample, index) => (
            <li key={`${String(index)}-${sample.subject}`} className={styles["sample"]}>
              <span className={styles["subject"]}>{sample.subject}</span>
              <span className={styles["sender"]}>{sample.sender}</span>
              <span className={styles["when"]}>{sample.when}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
