import { describe, expect, it } from "vitest";

import { NOTION_PAGES, notionLink } from "../../../test/demo/pages";

import { humanKind, KEPT_KINDS } from "./notion-kinds";
import {
  agoLabel,
  allNodes,
  choiceOf,
  flatten,
  importRequest,
  importTree,
  knownReason,
  lastSyncAt,
  notionUrl,
  toggle,
  type ImportPage,
} from "./notion";

const ids = (nodes: readonly { page: ImportPage }[]) => nodes.map((n) => n.page.notion_page);
const tree = importTree(NOTION_PAGES);
const [ROADMAP, OKRS, RETRO, NOTES, HIRING] = NOTION_PAGES.map((p) => p.notion_page) as [
  string,
  string,
  string,
  string,
  string,
];

describe("notionUrl", () => {
  it("is the page id without dashes", () => {
    expect(notionUrl("0192f1a0-0000-7000-8000-0000000007a1")).toBe(
      "https://www.notion.so/0192f1a00000700080000000000007a1",
    );
  });
});

describe("agoLabel", () => {
  const now = new Date("2026-10-25T10:00:00Z");
  const at = (ms: number) => new Date(now.getTime() - ms);

  it("says how long ago, in the reader's language", () => {
    expect(agoLabel(at(10_000), now, "en")).toBe("now");
    expect(agoLabel(at(60_000), now, "en")).toMatch(/^1 min/u);
    expect(agoLabel(at(3 * 60 * 60_000), now, "en")).toMatch(/^3 h/u);
    expect(agoLabel(at(2 * 24 * 60 * 60_000), now, "en")).toMatch(/^2 days? ago/u);
    expect(agoLabel(at(60_000), now, "pt-PT")).toContain("1");
  });

  it("never reads a clock a little ahead as the future", () => {
    expect(agoLabel(new Date(now.getTime() + 5000), now, "en")).toBe("now");
  });
});

describe("lastSyncAt", () => {
  it("is the newer of the two, or nothing", () => {
    const base = notionLink("p", ROADMAP, { last_inbound_at: null, last_outbound_at: null });
    expect(lastSyncAt(base)).toBeUndefined();
    expect(lastSyncAt({ ...base, last_inbound_at: "2026-10-25T09:00:00Z" })).toBe(
      "2026-10-25T09:00:00Z",
    );
    expect(
      lastSyncAt({
        ...base,
        last_inbound_at: "2026-10-25T09:00:00Z",
        last_outbound_at: "2026-10-25T09:30:00Z",
      }),
    ).toBe("2026-10-25T09:30:00Z");
  });
});

describe("knownReason", () => {
  it("knows the reasons it words and no others", () => {
    expect(knownReason("not_shared")).toBe("not_shared");
    expect(knownReason("something_new")).toBeUndefined();
    expect(knownReason(null)).toBeUndefined();
    expect(knownReason(undefined)).toBeUndefined();
  });
});

describe("importTree", () => {
  it("nests the pages under the listed page they are under", () => {
    expect(ids(tree)).toEqual([ROADMAP, NOTES, HIRING]);
    expect(ids(tree[0]?.children ?? [])).toEqual([OKRS, RETRO]);
    expect(allNodes(tree).map((n) => n.depth)).toEqual([0, 1, 1, 0, 0]);
    expect(ids(flatten(tree[0] as (typeof tree)[number]))).toEqual([ROADMAP, OKRS, RETRO]);
  });

  it("makes a root of a page whose parent is not listed, and survives a cycle", () => {
    const pages: ImportPage[] = [
      { notion_page: "a", title: "A", parent: "missing", sub_pages: 0, linked: false },
      { notion_page: "b", title: "B", parent: "c", sub_pages: 0, linked: false },
      { notion_page: "c", title: "C", parent: "b", sub_pages: 0, linked: false },
      { notion_page: "d", title: "D", parent: "d", sub_pages: 0, linked: false },
    ];
    // A page under itself, or two under each other, is not a tree: none is lost.
    const built = importTree(pages);
    expect(ids(allNodes(built)).toSorted()).toEqual(["a", "b", "c", "d"]);
  });
});

describe("ticking", () => {
  it("takes the sub-pages along, and they are not asked for on their own", () => {
    const ticked = toggle(tree, new Set(), ROADMAP);
    const choice = choiceOf(tree, ticked);
    expect([...choice.included].toSorted()).toEqual([ROADMAP, OKRS, RETRO].toSorted());
    expect(importRequest(tree, ticked)).toEqual([{ notion_page: ROADMAP, title: "Product roadmap" }]);
  });

  it("asks for a sub-page alone when only it is ticked", () => {
    const ticked = toggle(tree, new Set(), OKRS);
    expect(importRequest(tree, ticked)).toEqual([{ notion_page: OKRS, title: "Q4 OKRs" }]);
    expect(choiceOf(tree, ticked).included.size).toBe(1);
  });

  it("drops a ticked sub-page when its page is ticked, and unticks", () => {
    let ticked = toggle(tree, new Set(), OKRS);
    ticked = toggle(tree, ticked, ROADMAP);
    expect([...ticked]).toEqual([ROADMAP]);
    ticked = toggle(tree, ticked, ROADMAP);
    expect(ticked.size).toBe(0);
  });

  it("leaves a page that is synced already, and one that is not there", () => {
    expect(toggle(tree, new Set(), HIRING).size).toBe(0);
    expect(toggle(tree, new Set(), "nowhere").size).toBe(0);
    // A synced page under a ticked one is not counted or asked for.
    const pages: ImportPage[] = [
      { notion_page: "p", title: "P", parent: null, sub_pages: 1, linked: false },
      { notion_page: "q", title: "Q", parent: "p", sub_pages: 0, linked: true },
    ];
    const nested = importTree(pages);
    const ticked = new Set(["p", "q"]);
    expect(choiceOf(nested, ticked).included.has("q")).toBe(false);
    expect(importRequest(nested, ticked)).toEqual([{ notion_page: "p", title: "P" }]);
  });

  it("cuts a long title to what the server takes", () => {
    const long = importTree([
      { notion_page: "x", title: "t".repeat(300), parent: null, sub_pages: 0, linked: false },
    ]);
    expect(importRequest(long, new Set(["x"]))[0]?.title).toHaveLength(200);
  });
});

describe("the kinds kept in Notion", () => {
  it("reads an unknown type as words", () => {
    expect(humanKind("some_new_block")).toBe("Some new block");
    expect(humanKind("")).toBe("");
    expect(KEPT_KINDS).toContain("column_list");
  });
});
