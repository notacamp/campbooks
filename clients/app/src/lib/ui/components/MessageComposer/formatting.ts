/**
 * The basic formatting a message may carry, as marks in its text: the
 * server renders `**bold**`, `*italic*`, `- ` bullets and `1. ` numbered
 * items (and quotes and bare links) into the HTML that is sent.
 */
export type Format = "bold" | "italic" | "bullets" | "numbers";

/** A text and its selection. */
export interface Edit {
  text: string;
  start: number;
  end: number;
}

const MARKS: Record<"bold" | "italic", string> = { bold: "**", italic: "*" };

/** Wrap the selection in a mark, or unwrap it when it is wrapped already. */
function wrap({ text, start, end }: Edit, mark: string): Edit {
  const before = text.slice(0, start);
  const selected = text.slice(start, end);
  const after = text.slice(end);
  if (before.endsWith(mark) && after.startsWith(mark)) {
    return {
      text: before.slice(0, -mark.length) + selected + after.slice(mark.length),
      start: start - mark.length,
      end: end - mark.length,
    };
  }
  return {
    text: `${before}${mark}${selected}${mark}${after}`,
    start: start + mark.length,
    end: end + mark.length,
  };
}

/** Start every line the selection touches with a list mark. */
function list({ text, start, end }: Edit, numbered: boolean): Edit {
  const lineStart = text.lastIndexOf("\n", start - 1) + 1;
  const nextBreak = text.indexOf("\n", end);
  const lineEnd = nextBreak === -1 ? text.length : nextBreak;
  const lines = text.slice(lineStart, lineEnd).split("\n");
  const mark = (index: number) => (numbered ? `${String(index + 1)}. ` : "- ");
  const marked = lines.map((line, index) => `${mark(index)}${line}`).join("\n");
  const added = marked.length - (lineEnd - lineStart);
  return {
    text: text.slice(0, lineStart) + marked + text.slice(lineEnd),
    start: start + mark(0).length,
    end: end + added,
  };
}

/** Apply `format` to the selection. */
export function applyFormat(edit: Edit, format: Format): Edit {
  switch (format) {
    case "bold":
    case "italic":
      return wrap(edit, MARKS[format]);
    case "bullets":
      return list(edit, false);
    case "numbers":
      return list(edit, true);
  }
}
