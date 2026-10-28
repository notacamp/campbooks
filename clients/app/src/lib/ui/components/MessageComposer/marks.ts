/**
 * A draft's text is plain text with marks (`**bold**`, `*italic*`,
 * `~~strikethrough~~`, `[words](url)`, `- ` and `1. ` items, `> ` quotes),
 * and only the server turns it into HTML (`conversations`' `domain/format.rs`).
 * The editor holds a document; these two functions convert between the two.
 * Both read `crates/modules/conversations/tests/fixtures/marks.json`, which
 * the server's tests read too, so the sides cannot drift apart.
 */

/** The part of a ProseMirror node the composer's schema uses. */
export interface DocNode {
  type: string;
  text?: string | undefined;
  attrs?: Record<string, unknown> | undefined;
  marks?: { type: string; attrs?: Record<string, unknown> | undefined }[] | undefined;
  content?: DocNode[] | undefined;
}

type Style = "strike" | "bold" | "italic";
/** Outermost first: the order marks open in. */
const STYLES: readonly Style[] = ["strike", "bold", "italic"];
const MARKER: Record<Style, string> = { strike: "~~", bold: "**", italic: "*" };

/** The address a link may go to: the server links no other. */
export function isLinkable(url: string): boolean {
  return /^(https?:\/\/|mailto:)[^\s<>"]+$/iu.test(url);
}

/** What the member typed as an address, made one if it plainly is, else unchanged. */
export function normalizeAddress(input: string): string {
  const text = input.trim();
  if (/^[^\s@:/]+@[^\s@:/]+\.[^\s@:/]+$/u.test(text)) {
    return `mailto:${text}`;
  }
  if (/^[^\s:/@]+\.[^\s:/@]{2,}(?:[/?#]\S*)?$/u.test(text)) {
    return `https://${text}`;
  }
  return text;
}

// ---------------------------------------------------------------------------
// Document → marks

interface Fragment {
  text: string;
  styles: ReadonlySet<Style>;
  href: string | undefined;
}

/** A document as the marks it is saved as. */
export function toMarks(doc: DocNode): string {
  return blocksToMarks(doc.content ?? []);
}

function blocksToMarks(blocks: readonly DocNode[]): string {
  const parts: string[] = [];
  for (const block of blocks) {
    const text = blockToMarks(block);
    if (text !== "") {
      parts.push(text);
    }
  }
  return parts.join("\n\n");
}

function blockToMarks(block: DocNode): string {
  switch (block.type) {
    case "paragraph":
      return lines(block.content ?? [])
        .map((line) => lineToMarks(line))
        .join("\n");
    case "bulletList":
    case "orderedList":
      return listToMarks(block);
    case "blockquote":
      return blocksToMarks(block.content ?? [])
        .split("\n")
        .map((line) => (line === "" ? ">" : `> ${line}`))
        .join("\n");
    default:
      // Anything the schema holds no mark for: its text, as a paragraph.
      return lines(flattenText(block)).map(lineToMarks).join("\n");
  }
}

function flattenText(node: DocNode): DocNode[] {
  if (node.type === "text" || node.type === "hardBreak") {
    return [node];
  }
  return (node.content ?? []).flatMap(flattenText);
}

/** The items of a list, one level: deeper lists are flattened into it. */
function listToMarks(list: DocNode): string {
  const ordered = list.type === "orderedList";
  const items: string[] = [];
  const visit = (node: DocNode) => {
    for (const item of node.content ?? []) {
      const own = (item.content ?? []).filter(
        (child) => child.type !== "bulletList" && child.type !== "orderedList",
      );
      // One line per item: its paragraphs and line breaks read as spaces.
      const line = lines(own.flatMap((p) => joinParagraphs(p)))
        .map((fragments) => lineToMarks(fragments, false))
        .join(" ");
      items.push(line);
      for (const child of item.content ?? []) {
        if (child.type === "bulletList" || child.type === "orderedList") {
          visit(child);
        }
      }
    }
  };
  visit(list);
  return items.map((item, at) => `${ordered ? `${String(at + 1)}. ` : "- "}${item}`).join("\n");
}

function joinParagraphs(paragraph: DocNode): DocNode[] {
  return [...flattenText(paragraph), { type: "hardBreak" }];
}

/** A paragraph's inline nodes, split at its line breaks, as fragments. */
function lines(nodes: readonly DocNode[]): Fragment[][] {
  const result: Fragment[][] = [[]];
  for (const node of nodes) {
    if (node.type === "hardBreak") {
      result.push([]);
    } else if (node.type === "text" && node.text !== undefined && node.text !== "") {
      const types = new Set((node.marks ?? []).map((mark) => mark.type));
      const link = node.marks?.find((mark) => mark.type === "link");
      const href = link?.attrs?.["href"];
      result[result.length - 1]?.push({
        text: node.text,
        styles: new Set(STYLES.filter((style) => types.has(style))),
        href: typeof href === "string" && isLinkable(href) ? href : undefined,
      });
    }
  }
  // A list item's trailing break (added to join paragraphs) is no line.
  while (result.length > 1 && result[result.length - 1]?.length === 0) {
    result.pop();
  }
  return result;
}

const intersect = (a: ReadonlySet<Style>, b: ReadonlySet<Style> | undefined): Set<Style> =>
  new Set([...a].filter((style) => b?.has(style) === true));

/**
 * Spaces at the edge of a mark stay outside it (`** a**` is not bold), so a
 * space only keeps a style its neighbours across the edge also carry.
 */
function trimEdges(line: readonly Fragment[]): Fragment[] {
  const out: Fragment[] = [];
  line.forEach((fragment, at) => {
    const before = line[at - 1]?.styles;
    const after = line[at + 1]?.styles;
    const lead = /^\s*/u.exec(fragment.text)?.[0] ?? "";
    if (lead.length === fragment.text.length) {
      out.push({ ...fragment, styles: intersect(intersect(fragment.styles, before), after) });
      return;
    }
    const trail = /\s*$/u.exec(fragment.text)?.[0] ?? "";
    const core = fragment.text.slice(lead.length, fragment.text.length - trail.length);
    if (lead !== "") {
      out.push({ ...fragment, text: lead, styles: intersect(fragment.styles, before) });
    }
    out.push({ ...fragment, text: core });
    if (trail !== "") {
      out.push({ ...fragment, text: trail, styles: intersect(fragment.styles, after) });
    }
  });
  return out.reduce<Fragment[]>((merged, fragment) => {
    const last = merged[merged.length - 1];
    if (
      last !== undefined &&
      last.href === fragment.href &&
      last.styles.size === fragment.styles.size &&
      [...last.styles].every((style) => fragment.styles.has(style))
    ) {
      last.text += fragment.text;
    } else {
      merged.push({ ...fragment });
    }
    return merged;
  }, []);
}

/** The escaping of one line, which knows what it has written so far. */
class Escaper {
  /** Anything but a digit written yet: a line's start is over. */
  private written = false;
  private digits = 0;
  private at = 0;

  constructor(
    private readonly plain: string,
    private readonly leading: boolean,
  ) {}

  /** Marks and brackets written around the text count as written. */
  mark(text: string): string {
    this.written = true;
    return text;
  }

  text(text: string): string {
    let out = "";
    for (const char of text) {
      this.at += char.length;
      const next = this.plain[this.at];
      const start = this.leading && !this.written;
      if ("*~[]\\".includes(char)) {
        out += `\\${char}`;
      } else if (start && this.digits === 0 && (char === "-" || char === ">")) {
        out += `\\${char}`;
      } else if (start && this.digits > 0 && char === "." && (next === " " || next === undefined)) {
        out += "\\.";
      } else {
        out += char;
      }
      if (start && /\d/u.test(char)) {
        this.digits += 1;
      } else {
        this.written = true;
      }
    }
    return out;
  }
}

/** One line of a paragraph (or one item) as marks. `leading` escapes what would read as a list or quote. */
function lineToMarks(line: readonly Fragment[], leading = true): string {
  const fragments = trimEdges(line);
  const escaper = new Escaper(fragments.map((f) => f.text).join(""), leading);
  let out = "";
  let at = 0;
  while (at < fragments.length) {
    const fragment = fragments[at];
    if (fragment === undefined) {
      break;
    }
    if (fragment.href === undefined) {
      let end = at;
      while (fragments[end + 1] !== undefined && fragments[end + 1]?.href === undefined) {
        end += 1;
      }
      out += styled(fragments.slice(at, end + 1), escaper);
      at = end + 1;
      continue;
    }
    let end = at;
    while (fragments[end + 1]?.href === fragment.href) {
      end += 1;
    }
    const group = fragments.slice(at, end + 1);
    const words = group.map((f) => f.text).join("");
    const bare = group.every((f) => f.styles.size === 0) && bareAddress(words, fragment.href);
    if (bare) {
      out += escaper.mark(words);
    } else {
      out += escaper.mark("[");
      out += styled(group, escaper);
      out += escaper.mark(`](${encodeAddress(fragment.href)})`);
    }
    at = end + 1;
  }
  return out;
}

/** An address that reads as a bare link: the server links it itself. */
function bareAddress(words: string, href: string): boolean {
  return (
    words === href && /^https?:\/\/[^\s<>"'*~[\]\\]+$/u.test(href) && !/[.,;:!?)*]$/u.test(href)
  );
}

/** An address that cannot end the `(…)` or break out of it. */
function encodeAddress(href: string): string {
  return href.replaceAll(
    /[\s)<>"]/gu,
    (char) => `%${char.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0")}`,
  );
}

/** Fragments with their styles, as marks, keeping the markers nested. */
function styled(fragments: readonly Fragment[], escaper: Escaper): string {
  let out = "";
  const open: Style[] = [];
  for (const fragment of fragments) {
    const unwanted = open.findIndex((style) => !fragment.styles.has(style));
    if (unwanted >= 0) {
      const above = open.splice(unwanted);
      for (const style of [...above].reverse()) {
        out += escaper.mark(MARKER[style]);
      }
      for (const style of above) {
        if (fragment.styles.has(style)) {
          open.push(style);
          out += escaper.mark(MARKER[style]);
        }
      }
    }
    for (const style of STYLES) {
      if (fragment.styles.has(style) && !open.includes(style)) {
        open.push(style);
        out += escaper.mark(MARKER[style]);
      }
    }
    out += escaper.text(fragment.text);
  }
  for (const style of [...open].reverse()) {
    out += escaper.mark(MARKER[style]);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Marks → document

/** The document a draft's marks stand for (the server's reading, for the editor). */
export function fromMarks(text: string): DocNode {
  const content = blocksFrom(text.split(/\r?\n/u));
  return { type: "doc", content: content.length === 0 ? [{ type: "paragraph" }] : content };
}

const bulletItem = (line: string): string | undefined =>
  line.startsWith("- ") || line.startsWith("* ") ? line.slice(2) : undefined;

const numberedItem = (line: string): string | undefined => {
  const match = /^(\d{1,9})\. /u.exec(line);
  return match === null ? undefined : line.slice(match[0].length);
};

const isQuote = (line: string) => line.startsWith(">");

function blocksFrom(all: readonly string[]): DocNode[] {
  const blocks: DocNode[] = [];
  const run = (start: number, keep: (line: string) => boolean) => {
    let end = start;
    while (end < all.length && keep(all[end] ?? "")) {
      end += 1;
    }
    return end;
  };
  let i = 0;
  while (i < all.length) {
    const line = all[i] ?? "";
    if (line.trim() === "") {
      i += 1;
    } else if (isQuote(line)) {
      const end = run(i, isQuote);
      const inner = all.slice(i, end).map((l) => (l.startsWith("> ") ? l.slice(2) : l.slice(1)));
      const content = blocksFrom(inner);
      blocks.push({
        type: "blockquote",
        content: content.length === 0 ? [{ type: "paragraph" }] : content,
      });
      i = end;
    } else if (bulletItem(line) !== undefined) {
      const end = run(i, (l) => bulletItem(l) !== undefined);
      blocks.push(
        listFrom(
          "bulletList",
          all.slice(i, end).map((l) => bulletItem(l) ?? ""),
        ),
      );
      i = end;
    } else if (numberedItem(line) !== undefined) {
      const end = run(i, (l) => numberedItem(l) !== undefined);
      blocks.push(
        listFrom(
          "orderedList",
          all.slice(i, end).map((l) => numberedItem(l) ?? ""),
        ),
      );
      i = end;
    } else {
      const end = run(
        i,
        (l) =>
          l.trim() !== "" &&
          !isQuote(l) &&
          bulletItem(l) === undefined &&
          numberedItem(l) === undefined,
      );
      const content: DocNode[] = [];
      all.slice(i, end).forEach((l, at) => {
        if (at > 0) {
          content.push({ type: "hardBreak" });
        }
        content.push(...inlineNodes(l));
      });
      blocks.push(content.length === 0 ? { type: "paragraph" } : { type: "paragraph", content });
      i = end;
    }
  }
  return blocks;
}

function listFrom(type: "bulletList" | "orderedList", items: readonly string[]): DocNode {
  return {
    type,
    content: items.map((item) => {
      const content = inlineNodes(item);
      return {
        type: "listItem",
        content: [content.length === 0 ? { type: "paragraph" } : { type: "paragraph", content }],
      };
    }),
  };
}

interface Run {
  text: string;
  styles: ReadonlySet<Style>;
  href: string | undefined;
}

function inlineNodes(line: string): DocNode[] {
  const runs: Run[] = [];
  parseInline(line, true, undefined, runs);
  return runs.map(({ text, styles, href }) => {
    const marks: NonNullable<DocNode["marks"]> = [];
    for (const style of ["bold", "italic", "strike"] as const) {
      if (styles.has(style)) {
        marks.push({ type: style });
      }
    }
    if (href !== undefined) {
      marks.push({ type: "link", attrs: { href } });
    }
    return marks.length === 0 ? { type: "text", text } : { type: "text", text, marks };
  });
}

/** `[words](url)` at the start of `text`: the words, the address and the rest. */
function wordedLink(text: string): { words: string; url: string; rest: string } | undefined {
  if (!text.startsWith("[")) {
    return undefined;
  }
  let end = -1;
  for (let i = 1; i < text.length; i += 1) {
    if (text[i] === "\\") {
      i += 1;
    } else if (text[i] === "]") {
      end = i;
      break;
    }
  }
  if (end < 0 || text[end + 1] !== "(") {
    return undefined;
  }
  const close = text.indexOf(")", end + 2);
  if (close < 0) {
    return undefined;
  }
  const words = text.slice(1, end);
  const url = text.slice(end + 2, close);
  return words !== "" && isLinkable(url) ? { words, url, rest: text.slice(close + 1) } : undefined;
}

/** An escaped mark character, anywhere. */
const escapedChar = (text: string): string | undefined =>
  text[0] === "\\" && text[1] !== undefined && "*~[]\\".includes(text[1]) ? text[1] : undefined;

/** An escaped leading `-`, `>` or `1.`: what it stands for and how much it takes. */
function leadingEscape(text: string): { lit: string; length: number } | undefined {
  const digits = /^(\d{1,9})\\\./u.exec(text);
  if (digits?.[1] !== undefined) {
    return { lit: `${digits[1]}.`, length: digits[0].length };
  }
  if (text[0] === "\\" && (text[1] === "-" || text[1] === ">")) {
    return { lit: text[1], length: 2 };
  }
  return undefined;
}

/** Whether `pat` is in `text`, not counting a character a `\` escapes. */
function hasUnescaped(text: string, pat: string): boolean {
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === "\\") {
      i += 1;
    } else if (text.startsWith(pat, i)) {
      return true;
    }
  }
  return false;
}

/** Whether a single `*` later on the line can close an italic run opened here. */
function closesItalic(after: string): boolean {
  if (after === "" || after.startsWith(" ") || after.startsWith("*")) {
    return false;
  }
  for (let i = 1; i < after.length; i += 1) {
    if (after[i] === "\\") {
      i += 1;
      continue;
    }
    if (
      after[i] === "*" &&
      after[i - 1] !== " " &&
      after[i - 1] !== "*" &&
      after[i - 1] !== "\\" &&
      (after[i + 1] !== "*" || after.startsWith("***", i))
    ) {
      return true;
    }
  }
  return false;
}

/** The server's inline reading of one line (own text), as runs of text. */
function parseInline(
  line: string,
  first: boolean,
  href: string | undefined,
  runs: Run[],
  links = true,
): void {
  const open = new Set<Style>();
  let rest = line;
  let atStart = first;
  const push = (text: string) => {
    const last = runs[runs.length - 1];
    if (
      last !== undefined &&
      last.href === href &&
      last.styles.size === open.size &&
      [...open].every((style) => last.styles.has(style))
    ) {
      last.text += text;
    } else {
      runs.push({ text, styles: new Set(open), href });
    }
  };
  const toggle = (style: Style) => {
    if (open.has(style)) {
      open.delete(style);
    } else {
      open.add(style);
    }
  };
  while (rest !== "") {
    const start = atStart;
    atStart = false;
    if (start) {
      const lead = leadingEscape(rest);
      if (lead !== undefined) {
        push(lead.lit);
        rest = rest.slice(lead.length);
        continue;
      }
    }
    const escaped = escapedChar(rest);
    if (escaped !== undefined) {
      push(escaped);
      rest = rest.slice(2);
      continue;
    }
    if (rest.startsWith("~~") && (open.has("strike") || hasUnescaped(rest.slice(2), "~~"))) {
      toggle("strike");
      rest = rest.slice(2);
      continue;
    }
    if (links) {
      const link = wordedLink(rest);
      if (link !== undefined) {
        parseInline(link.words, false, link.url, runs, false);
        rest = link.rest;
        continue;
      }
    }
    if (rest.startsWith("***")) {
      const after = rest.slice(3);
      if (open.has("bold") && open.has("italic")) {
        open.delete("bold");
        open.delete("italic");
        rest = after;
        continue;
      }
      if (open.size === 0 && hasUnescaped(after, "***")) {
        open.add("bold");
        open.add("italic");
        rest = after;
        continue;
      }
    }
    if (rest.startsWith("**") && (open.has("bold") || hasUnescaped(rest.slice(2), "**"))) {
      toggle("bold");
      rest = rest.slice(2);
      continue;
    }
    if (rest.startsWith("*") && (open.has("italic") || closesItalic(rest.slice(1)))) {
      toggle("italic");
      rest = rest.slice(1);
      continue;
    }
    const char = String.fromCodePoint(rest.codePointAt(0) ?? 0);
    push(char);
    rest = rest.slice(char.length);
  }
}
