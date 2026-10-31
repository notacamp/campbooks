import type { RecordRef } from "~/lib/api";

import type { Proposal } from "./proposals";

/** The most Markdown a section may have: 16 KiB (`pages.append`, `pages.create`). */
export const MARKDOWN_MAX_BYTES = 16 * 1024;

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

/**
 * What a page proposal's card shows, and so what the member confirms: where
 * the section goes, who will read it, whether Notion gets it too, and the
 * whole Markdown. Read from the proposal as the server built it, never from
 * the model's words (the readers and the Notion line are the page's own
 * facts).
 */
export interface PageProposalView {
  /** A section for an existing page, or a new page. */
  kind: "append" | "create";
  /** The page's title, or for a new page the title proposed. */
  title: string;
  /** For a new page, the page it goes under (it takes its readers from it). */
  parent: string | undefined;
  /** Who reads it, as the page's effective audience says. */
  audience: "only_you" | "named" | "everyone";
  /** The teammates named. */
  readers: string[];
  /** How many more read it than are named. */
  others: number;
  /** What is added is sent to Notion too. */
  notion: boolean;
  /** The whole section, as Markdown. */
  markdown: string;
  /** The threads (or tasks, events, pages) it draws on. */
  sources: RecordRef[];
  /** The existing page (append only). */
  pageId: string | undefined;
}

/**
 * The card's view of `proposal`, or `undefined` when it cannot say everything
 * a member must see before adding: no Markdown, no word on who reads the
 * page, or a readers line that contradicts itself. Such a proposal shows no
 * "Add to page".
 */
export function pageProposalOf(proposal: Proposal): PageProposalView | undefined {
  const action = proposal.action;
  if (action !== "append_section" && action !== "new_page") {
    return undefined;
  }
  const page = proposal.page;
  const markdown = proposal.detail?.markdown;
  if (
    page === undefined ||
    page === null ||
    typeof markdown !== "string" ||
    markdown.trim() === ""
  ) {
    return undefined;
  }
  const { audience, readers, others } = page as {
    audience: unknown;
    readers: unknown;
    others: unknown;
  };
  if (
    (audience !== "only_you" && audience !== "named" && audience !== "everyone") ||
    !Array.isArray(readers) ||
    !readers.every((name): name is string => typeof name === "string" && name.trim() !== "") ||
    typeof others !== "number" ||
    !Number.isInteger(others) ||
    others < 0 ||
    typeof page.notion !== "boolean"
  ) {
    return undefined;
  }
  // "Only you" must really be only you; "named" must name or count somebody.
  const somebody = readers.length + others > 0;
  if ((audience === "only_you" && somebody) || (audience === "named" && !somebody)) {
    return undefined;
  }
  const parent = page.parent ?? undefined;
  return {
    kind: action === "append_section" ? "append" : "create",
    title: page.title,
    parent: parent === "" ? undefined : parent,
    audience,
    readers,
    others,
    notion: page.notion,
    markdown,
    sources: proposal.detail?.sources ?? [],
    pageId: action === "append_section" ? (proposal.record?.id ?? undefined) : undefined,
  };
}

/** The Markdown's size in bytes, as the server counts it. */
export function markdownBytes(markdown: string): number {
  return new TextEncoder().encode(markdown).length;
}

/**
 * "Add to page" is offered only when the card shows everything being
 * confirmed: a proposal the card can state in full, whose text (as it stands
 * after an edit) is neither empty nor over the cap.
 */
export function mayAdd(view: PageProposalView | undefined, markdown: string): boolean {
  return (
    view !== undefined && markdown.trim() !== "" && markdownBytes(markdown) <= MARKDOWN_MAX_BYTES
  );
}

/**
 * Who will read it, as a sentence for the card: "Only you", "You, Rui and 2
 * others", "You and 3 others", "Everyone in the workspace". It never says
 * only you when anyone else reads the page.
 */
export function readersSentence(t: Translate, view: PageProposalView): string {
  if (view.audience === "everyone") {
    return t("chat.proposal.page.readers.everyone");
  }
  if (view.readers.length + view.others === 0) {
    return t("chat.proposal.page.readers.only");
  }
  const pieces = [
    t("chat.proposal.page.readers.you"),
    ...view.readers,
    ...(view.others > 0 ? [t("chat.proposal.page.readers.others", { count: view.others })] : []),
  ];
  const and = t("chat.proposal.page.readers.and");
  return pieces.length === 1
    ? (pieces[0] ?? "")
    : `${pieces.slice(0, -1).join(", ")} ${and} ${pieces.at(-1) ?? ""}`;
}
