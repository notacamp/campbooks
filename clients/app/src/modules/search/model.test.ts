import { afterEach, describe, expect, it } from "vitest";

import { writePref } from "~/lib/storage/prefs";

import {
  addToken,
  chipLabel,
  forgetSearches,
  hasFilter,
  recentSearches,
  rememberSearch,
  removeFilter,
  toggleFilter,
} from "./model";

describe("removing a filter from the query text", () => {
  it("removes the token the filter came from, leaving the words", () => {
    expect(
      removeFilter("invoice from:ana has:attachment", { operator: "from", value: "ana" }),
    ).toBe("invoice has:attachment");
  });

  it("matches ignoring case, quotes and a leading minus", () => {
    expect(removeFilter('lease -From:"Ana L" new', { operator: "from", value: "ana l" })).toBe(
      "lease new",
    );
  });

  it("falls back to the operator when the server normalised the value", () => {
    expect(removeFilter("after:1/9/2026 lease", { operator: "after", value: "2026-09-01" })).toBe(
      "lease",
    );
  });

  it("leaves a phrase with a colon in quotes alone", () => {
    expect(removeFilter('"from: ana" invoice', { operator: "from", value: "ana" })).toBe(
      '"from: ana" invoice',
    );
  });

  it("changes nothing when no token matches", () => {
    expect(removeFilter("invoice", { operator: "from", value: "ana" })).toBe("invoice");
  });
});

describe("quick filters", () => {
  const filters = [{ operator: "has", value: "attachment" }];

  it("adds a token once and removes an applied filter", () => {
    expect(addToken("invoice", "is:unread")).toBe("invoice is:unread");
    expect(addToken("invoice is:unread", "is:unread")).toBe("invoice is:unread");
    expect(hasFilter(filters, "has", "attachment")).toBe(true);
    expect(toggleFilter("invoice has:attachment", filters, "has", "attachment")).toBe("invoice");
    expect(toggleFilter("invoice has:attachment", filters, "is", "unread")).toBe(
      "invoice has:attachment is:unread",
    );
  });
});

describe("chip labels", () => {
  const t = (key: string, options?: Record<string, unknown>) =>
    `${key}|${JSON.stringify(options ?? {})}`;

  it("names the known operators and falls back to operator and value", () => {
    expect(chipLabel(t, { operator: "has", value: "attachment" })).toBe("chip.attachment|{}");
    expect(chipLabel(t, { operator: "is", value: "unread" })).toBe("chip.is.unread|{}");
    expect(chipLabel(t, { operator: "from", value: "ana" })).toBe('chip.from|{"value":"ana"}');
    expect(chipLabel(t, { operator: "before", value: "2026-09-01" })).toContain("chip.date.before");
    expect(chipLabel(t, { operator: "weird", value: "x" })).toContain("chip.other");
  });
});

describe("recent searches", () => {
  afterEach(() => {
    forgetSearches();
  });

  it("keeps the newest first, once each, at most eight", () => {
    for (const query of ["a", "b", "c", "d", "e", "f", "g", "h", "i", "B "]) {
      rememberSearch(query);
    }

    const recent = recentSearches();
    expect(recent).toHaveLength(8);
    expect(recent[0]).toBe("B");
    expect(recent.filter((query) => query.toLowerCase() === "b")).toHaveLength(1);
  });

  it("ignores empty and over-long queries, and clears", () => {
    rememberSearch(" ".repeat(3));
    rememberSearch("x".repeat(513));
    expect(recentSearches()).toEqual([]);
    rememberSearch("lease");
    forgetSearches();
    expect(recentSearches()).toEqual([]);
  });

  it("treats damaged storage as empty", () => {
    writePref("searches", "{not json");
    expect(recentSearches()).toEqual([]);
    writePref("searches", '[1, "ok"]');
    expect(recentSearches()).toEqual(["ok"]);
  });
});
