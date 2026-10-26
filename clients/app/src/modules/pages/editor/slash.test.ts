import { describe, expect, it, vi } from "vitest";

import { filterSlash, runSlash, SLASH_IDS, SLASH_ITEMS, TABLE_SLASH_IDS, TURN_INTO } from "./slash";

describe("the slash menu", () => {
  it("has an item for every id, once", () => {
    expect(SLASH_ITEMS.map((item) => item.id)).toEqual([...SLASH_IDS]);
  });

  it("can turn a block into only blocks that are in the menu", () => {
    for (const id of TURN_INTO) {
      expect(SLASH_IDS).toContain(id);
    }
  });

  it("filters by what is typed, ignoring case and accents", () => {
    const names: Record<string, string> = { text: "Texto", heading1: "Título 1", todo: "Tarefa" };
    const name = (id: string) => names[id] ?? id;
    expect(filterSlash(SLASH_ITEMS, "TIT", name).map((item) => item.id)).toEqual(["heading1"]);
    expect(filterSlash(SLASH_ITEMS, "titulo", name).map((item) => item.id)).toEqual(["heading1"]);
    expect(filterSlash(SLASH_ITEMS, "", name)).toHaveLength(SLASH_ITEMS.length);
    expect(filterSlash(SLASH_ITEMS, "zzz", name)).toEqual([]);
  });
});

describe("the table blocks", () => {
  /** An editor that remembers what it was asked to do. */
  function fake() {
    const calls: string[] = [];
    const chain: Record<string, unknown> = {};
    for (const name of ["focus", "deleteRange", "insertContent", "setParagraph"]) {
      chain[name] = (...args: unknown[]) => {
        calls.push(
          `${name}${args.length > 1 || typeof args[0] === "string" ? `:${String(args[0])}` : ""}`,
        );
        return chain;
      };
    }
    chain["run"] = () => true;
    return { calls, editor: { chain: () => chain } as unknown as Parameters<typeof runSlash>[0] };
  }
  const helpers = () => ({
    attach: vi.fn(),
    mention: vi.fn(),
    subPage: vi.fn(),
    table: vi.fn(),
    tableLink: vi.fn(),
  });

  it("are in the menu, and are the only ones that need tables to be on", () => {
    expect(SLASH_IDS).toContain("table");
    expect(SLASH_IDS).toContain("tableLink");
    expect(TABLE_SLASH_IDS).toEqual(["table", "tableLink"]);
    expect(TURN_INTO).not.toContain("table");
  });

  it("make a table where the slash was", () => {
    const { editor } = fake();
    const h = helpers();
    runSlash(editor, { from: 1, to: 2 }, "table", h);
    expect(h.table).toHaveBeenCalledTimes(1);
    expect(h.tableLink).not.toHaveBeenCalled();
  });

  it("open the table picker with an @", () => {
    const { editor, calls } = fake();
    const h = helpers();
    runSlash(editor, { from: 1, to: 2 }, "tableLink", h);
    expect(calls).toContain("insertContent:@");
    expect(h.tableLink).toHaveBeenCalledTimes(1);
    expect(h.mention).not.toHaveBeenCalled();
  });
});
