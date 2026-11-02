import { Fragment, type ReactNode } from "react";

import { fromMarks, isLinkable, type DocNode } from "./marks";
import styles from "./SignatureText.module.css";

/** A text node with its bold, italic and link marks, as elements. */
function textOf(node: DocNode, key: number): ReactNode {
  let out: ReactNode = node.text ?? "";
  for (const mark of node.marks ?? []) {
    switch (mark.type) {
      case "bold": {
        out = <strong>{out}</strong>;
        break;
      }
      case "italic": {
        out = <em>{out}</em>;
        break;
      }
      case "strike": {
        out = <s>{out}</s>;
        break;
      }
      case "link": {
        const href = mark.attrs?.["href"];
        if (typeof href === "string" && isLinkable(href)) {
          out = (
            <a href={href} target="_blank" rel="noreferrer noopener">
              {out}
            </a>
          );
        }
        break;
      }
      default: {
        break;
      }
    }
  }
  return <Fragment key={key}>{out}</Fragment>;
}

/**
 * A signature's marks as they will read, not editable: paragraphs, line
 * breaks, bold, italic and links (a link opens in a new tab and only to a web
 * or mail address, as the server links no other).
 */
export function SignatureText({ marks }: { marks: string }) {
  const doc = fromMarks(marks);
  return (
    <div className={styles["text"]}>
      {(doc.content ?? []).map((block, at) => (
        <p key={at} className={styles["paragraph"]}>
          {(block.content ?? []).map((node, index) =>
            node.type === "hardBreak" ? <br key={index} /> : textOf(node, index),
          )}
        </p>
      ))}
    </div>
  );
}
