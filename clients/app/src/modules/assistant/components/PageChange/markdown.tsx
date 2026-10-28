import { Fragment, type ReactNode } from "react";

import { VisuallyHidden } from "~/lib/ui";

/**
 * Markdown as the page would show it, for the page change card: the same
 * small, fixed subset the server reads (`pages.append`), read line by line
 * into React elements, never into HTML. Whatever is not understood is text.
 * A link keeps only an https, http or mailto address; any other leaves the
 * words and drops the address.
 */

/** How deep emphasis and links may nest. */
const MAX_DEPTH = 6;

const SAFE_LINK = /^(https?:|mailto:)/iu;

type Block =
  | { kind: "heading"; level: 1 | 2 | 3; text: string }
  | { kind: "paragraph"; text: string }
  | { kind: "quote"; paragraphs: string[] }
  | { kind: "code"; text: string }
  | { kind: "rule" }
  | { kind: "list"; ordered: boolean; todo: boolean; items: { text: string; checked: boolean }[] };

const RULE = /^(?:-{3,}|\*{3,}|_{3,})$/u;
const HEADING = /^(#{1,6})\s+(.*)$/u;
const BULLET = /^\s{0,8}[-*+]\s+(.*)$/u;
const NUMBERED = /^\s{0,8}\d{1,9}[.)]\s+(.*)$/u;
const TODO = /^\[([ xX])\]\s+(.*)$/u;
const FENCE = /^ {0,3}```/u;

/** The blocks of `markdown`, in order. */
export function parseBlocks(markdown: string): Block[] {
  const lines = markdown.replaceAll("\t", " ".repeat(4)).split(/\r\n?|\n/u);
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  let quote: string[][] | undefined;
  const flush = () => {
    if (paragraph.length > 0) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
    if (quote !== undefined) {
      blocks.push({
        kind: "quote",
        paragraphs: quote.filter((part) => part.length > 0).map((part) => part.join(" ")),
      });
      quote = undefined;
    }
  };
  const item = (ordered: boolean, rest: string) => {
    flush();
    const todo = ordered ? null : TODO.exec(rest);
    const entry = {
      text: todo?.[2] ?? rest.trim(),
      checked: todo?.[1] !== undefined && todo[1] !== " ",
    };
    const last = blocks.at(-1);
    if (last?.kind === "list" && last.ordered === ordered && last.todo === (todo !== null)) {
      last.items.push(entry);
    } else {
      blocks.push({ kind: "list", ordered, todo: todo !== null, items: [entry] });
    }
  };
  let at = 0;
  while (at < lines.length) {
    const line = lines[at] ?? "";
    at += 1;
    const trimmed = line.trim();
    if (FENCE.test(line)) {
      flush();
      const code: string[] = [];
      while (at < lines.length) {
        const next = lines[at] ?? "";
        at += 1;
        if (next.trim() === "```") {
          break;
        }
        code.push(next);
      }
      blocks.push({ kind: "code", text: code.join("\n") });
    } else if (trimmed === "") {
      flush();
    } else if (RULE.test(trimmed)) {
      flush();
      blocks.push({ kind: "rule" });
    } else if (HEADING.test(trimmed)) {
      flush();
      const found = HEADING.exec(trimmed);
      blocks.push({
        kind: "heading",
        level: Math.min(found?.[1]?.length ?? 1, 3) as 1 | 2 | 3,
        text: found?.[2] ?? "",
      });
    } else if (trimmed.startsWith(">")) {
      const content = trimmed.slice(1).trim();
      if (quote === undefined) {
        flush();
        quote = [content === "" ? [] : [content]];
      } else if (content === "") {
        quote.push([]);
      } else {
        quote.at(-1)?.push(content);
      }
    } else if (BULLET.test(line)) {
      item(false, BULLET.exec(line)?.[1] ?? "");
    } else if (NUMBERED.test(line)) {
      item(true, NUMBERED.exec(line)?.[1] ?? "");
    } else {
      paragraph.push(trimmed);
    }
  }
  flush();
  return blocks;
}

/** Where `close` first closes the span that opens at `from`, or -1. */
function closing(text: string, from: number, close: string): number {
  return text.indexOf(close, from);
}

/** `text` as inline elements: bold, italic, strike, code and links. */
export function inline(text: string, depth = 0, key = "i"): ReactNode[] {
  const out: ReactNode[] = [];
  let plain = "";
  let index = 0;
  let count = 0;
  const push = (node: ReactNode) => {
    if (plain !== "") {
      out.push(plain);
      plain = "";
    }
    count += 1;
    out.push(<Fragment key={`${key}${String(count)}`}>{node}</Fragment>);
  };
  const nested = (inner: string) => inline(inner, depth + 1, `${key}${String(count)}-`);
  while (index < text.length) {
    const rest = text.slice(index);
    let done = false;
    if (depth < MAX_DEPTH) {
      if (rest.startsWith("`")) {
        const end = closing(text, index + 1, "`");
        if (end > index + 1) {
          push(<code>{text.slice(index + 1, end)}</code>);
          index = end + 1;
          done = true;
        }
      } else if (rest.startsWith("**") || rest.startsWith("__")) {
        const mark = rest.slice(0, 2);
        const end = closing(text, index + 2, mark);
        if (end > index + 2) {
          push(<strong>{nested(text.slice(index + 2, end))}</strong>);
          index = end + 2;
          done = true;
        }
      } else if (rest.startsWith("~~")) {
        const end = closing(text, index + 2, "~~");
        if (end > index + 2) {
          push(<s>{nested(text.slice(index + 2, end))}</s>);
          index = end + 2;
          done = true;
        }
      } else if (rest.startsWith("*") || rest.startsWith("_")) {
        const mark = rest.slice(0, 1);
        const end = closing(text, index + 1, mark);
        if (end > index + 1) {
          push(<em>{nested(text.slice(index + 1, end))}</em>);
          index = end + 1;
          done = true;
        }
      } else if (rest.startsWith("[")) {
        const label = closing(text, index + 1, "](");
        const end = label === -1 ? -1 : closing(text, label + 2, ")");
        if (label > index && end > label) {
          const href = text.slice(label + 2, end).trim();
          const words = nested(text.slice(index + 1, label));
          push(
            SAFE_LINK.test(href) ? (
              <a href={href} target="_blank" rel="noreferrer noopener">
                {words}
              </a>
            ) : (
              <>{words}</>
            ),
          );
          index = end + 1;
          done = true;
        }
      }
    }
    if (!done) {
      plain += text.charAt(index);
      index += 1;
    }
  }
  if (plain !== "") {
    out.push(plain);
  }
  return out;
}

/** The markdown of a section, drawn with the page's own block styles. */
export function PageMarkdown({
  markdown,
  todoLabels,
}: {
  markdown: string;
  /** What a to-do box says to a screen reader, ticked or not. */
  todoLabels: { done: string; open: string };
}) {
  const blocks = parseBlocks(markdown);
  return (
    <div className="tiptap">
      {blocks.map((block, index) => {
        const key = String(index);
        switch (block.kind) {
          case "heading": {
            const Tag = `h${String(block.level)}` as "h1" | "h2" | "h3";
            return <Tag key={key}>{inline(block.text, 0, key)}</Tag>;
          }
          case "paragraph": {
            return <p key={key}>{inline(block.text, 0, key)}</p>;
          }
          case "quote": {
            return (
              <blockquote key={key}>
                {block.paragraphs.map((part, at) => (
                  <p key={String(at)}>{inline(part, 0, `${key}-${String(at)}`)}</p>
                ))}
              </blockquote>
            );
          }
          case "code": {
            return (
              <pre key={key}>
                <code>{block.text}</code>
              </pre>
            );
          }
          case "rule": {
            return <hr key={key} />;
          }
          case "list": {
            const items = block.items.map((entry, at) =>
              block.todo ? (
                <li key={String(at)} data-type="taskItem" data-checked={String(entry.checked)}>
                  <label>
                    <VisuallyHidden>
                      {entry.checked ? todoLabels.done : todoLabels.open}
                    </VisuallyHidden>
                    <input type="checkbox" checked={entry.checked} disabled readOnly />
                  </label>
                  <div>
                    <p>{inline(entry.text, 0, `${key}-${String(at)}`)}</p>
                  </div>
                </li>
              ) : (
                <li key={String(at)}>
                  <p>{inline(entry.text, 0, `${key}-${String(at)}`)}</p>
                </li>
              ),
            );
            return block.ordered ? (
              <ol key={key}>{items}</ol>
            ) : block.todo ? (
              <ul key={key} data-type="taskList">
                {items}
              </ul>
            ) : (
              <ul key={key}>{items}</ul>
            );
          }
        }
      })}
    </div>
  );
}
