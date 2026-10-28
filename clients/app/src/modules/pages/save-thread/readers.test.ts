import { describe, expect, it } from "vitest";

import { joinNames, readersOf } from "./readers";
import { readersLine } from "./wording";

const ME = { user_id: "u-ana", you: true, display_name: "Ana Duarte" };
const RUI = { user_id: "u-rui", display_name: "Rui Costa" };
const SOFIA = { user_id: "u-sofia", display_name: null, email: "sofia@example.com" };
const members = [ME, RUI, SOFIA];

/** English as the app's strings say it, enough to read the wording. */
const t = (key: string, options: Record<string, unknown> = {}): string => {
  const count = Number(options["count"] ?? 0);
  const plural = (one: string, other: string) => (count === 1 ? one : other);
  const lines: Record<string, string> = {
    "saveToPage.readers.only": "Only you will read it",
    "saveToPage.readers.everyone": "Everyone in the workspace will read it",
    "saveToPage.readers.and": "and",
    "saveToPage.readers.others": plural(`${String(count)} other`, `${String(count)} others`),
    "saveToPage.readers.count": plural(
      `${String(count)} teammate will read it`,
      `${String(count)} teammates will read it`,
    ),
    "saveToPage.readers.named": `${String(options["names"])} will read it`,
  };
  return lines[key] ?? key;
};

describe("readersOf", () => {
  it("leaves the member out and names the rest by their workspace name", () => {
    const readers = readersOf(
      { everyone: false, notion: false, place: "private", readers: ["u-ana", "u-rui", "u-sofia"] },
      members,
    );
    expect(readers).toEqual({
      everyone: false,
      names: ["Rui Costa", "sofia@example.com"],
      others: 0,
    });
  });

  it("counts a reader it cannot name instead of showing an id", () => {
    const readers = readersOf(
      { everyone: false, notion: false, place: "private", readers: ["u-ana", "u-rui", "u-9f31"] },
      members,
    );
    expect(readers).toEqual({ everyone: false, names: ["Rui Costa"], others: 1 });
  });

  it("names at most five and counts the rest", () => {
    const many = Array.from({ length: 8 }, (_, index) => ({
      user_id: `u-${String(index)}`,
      display_name: `Person ${String(index)}`,
    }));
    const readers = readersOf(
      { everyone: false, notion: false, place: "private", readers: many.map((m) => m.user_id) },
      many,
    );
    expect(readers.names).toHaveLength(5);
    expect(readers.others).toBe(3);
  });

  it("says everyone for a workspace page", () => {
    expect(
      readersOf({ everyone: true, notion: false, place: "workspace", readers: [] }, members),
    ).toEqual({ everyone: true, names: [], others: 0 });
  });
});

describe("the readers line", () => {
  it("says only you, a name and a count, a count alone, and everyone", () => {
    expect(readersLine(t, { everyone: false, names: [], others: 0 })).toBe("Only you will read it");
    expect(readersLine(t, { everyone: false, names: ["Rui"], others: 0 })).toBe("Rui will read it");
    expect(readersLine(t, { everyone: false, names: ["Rui"], others: 2 })).toBe(
      "Rui and 2 others will read it",
    );
    expect(readersLine(t, { everyone: false, names: [], others: 3 })).toBe(
      "3 teammates will read it",
    );
    expect(readersLine(t, { everyone: true, names: [], others: 0 })).toBe(
      "Everyone in the workspace will read it",
    );
  });

  it("never calls a page private when somebody else reads it", () => {
    for (const readers of [
      { everyone: false, names: ["Rui"], others: 0 },
      { everyone: false, names: [], others: 1 },
      { everyone: true, names: [], others: 0 },
    ]) {
      expect(readersLine(t, readers).toLowerCase()).not.toMatch(/only you|private/u);
    }
  });
});

describe("joinNames", () => {
  it("joins with commas and the word for and", () => {
    expect(joinNames(["Rui"], "and")).toBe("Rui");
    expect(joinNames(["Rui", "Sofia"], "and")).toBe("Rui and Sofia");
    expect(joinNames(["Rui", "Sofia", "2 others"], "e")).toBe("Rui, Sofia e 2 others");
  });
});
