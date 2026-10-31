import { describe, expect, it } from "vitest";

import { KICKOFF_NOTES, NEW_PAGE_PROPOSAL, pageProposal } from "../../../../test/demo/assistant";

import {
  MARKDOWN_MAX_BYTES,
  markdownBytes,
  mayAdd,
  pageProposalOf,
  readersSentence,
} from "./page-proposal";

const t = (key: string, options: Record<string, unknown> = {}) => {
  const count = Number(options["count"] ?? 0);
  const words: Record<string, string> = {
    "chat.proposal.page.readers.everyone": "Everyone in the workspace",
    "chat.proposal.page.readers.only": "Only you",
    "chat.proposal.page.readers.you": "You",
    "chat.proposal.page.readers.and": "and",
    "chat.proposal.page.readers.others": count === 1 ? "1 other" : `${String(count)} others`,
  };
  return words[key] ?? key;
};

const sentence = (over: Parameters<typeof pageProposal>[0]) => {
  const view = pageProposalOf(pageProposal(over));
  return view === undefined ? undefined : readersSentence(t, view);
};

describe("pageProposalOf", () => {
  it("reads a section for a page: where, who reads it, Notion, the text and the sources", () => {
    const view = pageProposalOf(pageProposal());
    expect(view).toMatchObject({
      kind: "append",
      title: "Client onboarding",
      audience: "only_you",
      readers: [],
      others: 0,
      notion: false,
      markdown: KICKOFF_NOTES,
    });
    expect(view?.sources).toHaveLength(1);
    expect(view?.pageId).toBe("0192f1a0-0000-7000-8000-000000000907");
  });

  it("reads a new page with the page it goes under", () => {
    expect(pageProposalOf(NEW_PAGE_PROPOSAL)).toMatchObject({
      kind: "create",
      title: "Kickoff notes",
      parent: "Acme Studio",
      readers: ["Rui Matos"],
    });
  });

  it("says nothing of a change that is not a page's", () => {
    expect(pageProposalOf({ ...pageProposal(), action: "archive" })).toBeUndefined();
  });

  it("gives no view without the Markdown", () => {
    expect(pageProposalOf(pageProposal({ markdown: null }))).toBeUndefined();
    expect(pageProposalOf(pageProposal({ markdown: "  \n" }))).toBeUndefined();
  });

  it("gives no view without the page's readers", () => {
    expect(pageProposalOf(pageProposal({ page: null }))).toBeUndefined();
    const broken = pageProposal();
    const facts = broken.page ?? {
      title: "",
      audience: "only_you" as const,
      readers: [],
      others: 0,
      notion: false,
    };
    expect(
      pageProposalOf({
        ...broken,
        page: { ...facts, readers: undefined as unknown as string[] },
      }),
    ).toBeUndefined();
    expect(
      pageProposalOf({
        ...broken,
        page: { ...facts, audience: "someone" as unknown as "named" },
      }),
    ).toBeUndefined();
    expect(
      pageProposalOf({
        ...broken,
        page: { ...facts, notion: undefined as unknown as boolean },
      }),
    ).toBeUndefined();
  });

  it("gives no view when the readers contradict themselves", () => {
    const named = {
      title: "Acme",
      audience: "named" as const,
      readers: [],
      others: 0,
      notion: false,
    };
    expect(pageProposalOf(pageProposal({ page: named }))).toBeUndefined();
    const lying = { ...named, audience: "only_you" as const, readers: ["Rui Matos"] };
    expect(pageProposalOf(pageProposal({ page: lying }))).toBeUndefined();
  });
});

describe("mayAdd (no confirming without everything)", () => {
  it("needs a view, and text that is neither empty nor over 16 KiB", () => {
    const view = pageProposalOf(pageProposal());
    expect(mayAdd(view, KICKOFF_NOTES)).toBe(true);
    expect(mayAdd(undefined, KICKOFF_NOTES)).toBe(false);
    expect(mayAdd(view, "  ")).toBe(false);
    expect(mayAdd(view, "a".repeat(MARKDOWN_MAX_BYTES))).toBe(true);
    expect(mayAdd(view, "a".repeat(MARKDOWN_MAX_BYTES + 1))).toBe(false);
  });

  it("counts bytes, as the server does", () => {
    expect(markdownBytes("é")).toBe(2);
    expect(mayAdd(pageProposalOf(pageProposal()), "é".repeat(MARKDOWN_MAX_BYTES / 2 + 1))).toBe(
      false,
    );
  });
});

describe("readersSentence", () => {
  const named = (readers: string[], others: number) => ({
    title: "Acme",
    audience: "named" as const,
    readers,
    others,
    notion: false,
  });

  it("says only you, you and a name, a count, and everyone", () => {
    expect(sentence({})).toBe("Only you");
    expect(sentence({ page: named(["Rui Matos"], 0) })).toBe("You and Rui Matos");
    expect(sentence({ page: named(["Rui Matos"], 2) })).toBe("You, Rui Matos and 2 others");
    expect(sentence({ page: named([], 3) })).toBe("You and 3 others");
    expect(sentence({ page: { ...named([], 0), audience: "everyone", others: 12 } })).toBe(
      "Everyone in the workspace",
    );
  });

  it("never says only you when anyone else reads the page", () => {
    for (const page of [named(["Rui Matos"], 0), named([], 1), named(["Rui Matos"], 4)]) {
      expect(sentence({ page })).not.toMatch(/only you|private/iu);
    }
  });
});
