import { describe, expect, it } from "vitest";

import { filterSlash, SLASH_IDS, SLASH_ITEMS, TURN_INTO } from "./slash";

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
