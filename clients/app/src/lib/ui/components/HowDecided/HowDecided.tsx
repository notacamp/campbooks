import { useTranslation } from "react-i18next";

import { Disclosure } from "@notacamp/ui";

import styles from "./HowDecided.module.css";

export interface HowDecidedRow {
  /** The question, in words: "Needs a reply". */
  question: string;
  /** The answer and its reasons, in words: "Yes. It asks you to reply (“could you confirm”)." */
  answer: string;
  /** Where the numbers live: "Rules · 0.90 · acts from 0.88". */
  source: string;
}

export interface HowDecidedProps {
  rows: HowDecidedRow[];
  /** One line under the rows (how answers are held to their thresholds). */
  footnote?: string | undefined;
}

/**
 * "How Campbooks decided": closed by default, and the only place the
 * assistant's numbers appear. Each row says the answer in words first.
 */
export function HowDecided({ rows, footnote }: HowDecidedProps) {
  const { t } = useTranslation("ui");
  if (rows.length === 0) {
    return null;
  }
  return (
    <Disclosure summary={t("howDecided.title")}>
      <dl className={styles["rows"]}>
        {rows.map((row, index) => (
          <div key={`${String(index)}-${row.question}`} className={styles["row"]}>
            <dt className={styles["question"]}>{row.question}</dt>
            <dd className={styles["answer"]}>{row.answer}</dd>
            <dd className={styles["source"]}>{row.source}</dd>
          </div>
        ))}
      </dl>
      {footnote === undefined ? null : <p className={styles["footnote"]}>{footnote}</p>}
    </Disclosure>
  );
}
