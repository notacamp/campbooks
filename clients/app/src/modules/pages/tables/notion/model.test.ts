import { describe, expect, it } from "vitest";

import type { PropertyType, PropertyView } from "../types";

import {
  campbooksOnly,
  cellConflicts,
  cellKey,
  columnConflicts,
  columnDifferences,
  columnVersion,
  isSyncing,
  lockOf,
  markOf,
  reachesNotion,
  staysInCampbooks,
  type TableConflict,
  type TableSync,
} from "./model";

const property = (type: PropertyType): PropertyView => ({
  id: `p-${type}`,
  name: type,
  type,
  config: {},
  position: "a0",
  can_revert: false,
});

const conflict = (over: Partial<TableConflict>): TableConflict => ({
  id: "c1",
  kind: "cell",
  property: "p1",
  row: "r1",
  ours: "a",
  theirs: "b",
  ours_at: "2026-10-27T10:00:00Z",
  theirs_at: "2026-10-27T10:05:00Z",
  ...over,
});

describe("what a column of a synced table is", () => {
  it("locks exactly the reserved Notion types, each for its own reason", () => {
    expect(lockOf("notion_formula")).toBe("formula");
    expect(lockOf("notion_rollup")).toBe("rollup");
    expect(lockOf("notion_people")).toBe("people");
    expect(lockOf("external")).toBe("external");
    expect(lockOf("text")).toBeUndefined();
    expect(lockOf("status")).toBeUndefined();
  });

  it("sends what Notion can make or own, and keeps the rest in Campbooks", () => {
    for (const type of [
      "text",
      "number",
      "select",
      "multi_select",
      "date",
      "checkbox",
      "url",
    ] as const) {
      expect(reachesNotion(type)).toBe(true);
      expect(staysInCampbooks(type)).toBe(false);
    }
    for (const type of ["notion_formula", "notion_people", "external"] as const) {
      expect(reachesNotion(type)).toBe(true);
    }
    for (const type of [
      "person",
      "files",
      "relation",
      "campbooks_link",
      "created",
      "edited",
      "unique_id",
    ] as const) {
      expect(reachesNotion(type)).toBe(false);
      expect(campbooksOnly(type)).toBe(true);
    }
  });

  it("keeps a status column made here in Campbooks without calling it Campbooks only", () => {
    expect(staysInCampbooks("status")).toBe(true);
    expect(campbooksOnly("status")).toBe(false);
    expect(markOf(property("status"))).toEqual({ kind: "none" });
  });

  it("marks a column by what it is", () => {
    expect(markOf(property("notion_rollup"))).toEqual({ kind: "locked", lock: "rollup" });
    expect(markOf(property("campbooks_link"))).toEqual({ kind: "campbooks_only" });
    expect(markOf(property("number"))).toEqual({ kind: "none" });
  });
});

describe("the conflicts of a synced table", () => {
  it("finds the cell conflicts by row and column, and the column ones apart", () => {
    const cell = conflict({ id: "c1", row: "r1", property: "p1" });
    const column = conflict({ id: "c2", kind: "column", row: null, property: "p2" });
    expect(cellConflicts([cell, column]).get(cellKey("r1", "p1"))).toBe(cell);
    expect(cellConflicts([cell, column]).size).toBe(1);
    expect(columnConflicts([cell, column])).toEqual([column]);
  });

  it("reads a version of a column whatever the server put there", () => {
    expect(
      columnVersion({ name: "Deal size", type: "number", config: { options: [{}, {}] } }),
    ).toEqual({ name: "Deal size", type: "number", options: 2 });
    expect(columnVersion(null)).toEqual({ name: "", type: undefined, options: 0 });
    expect(columnVersion("nonsense")).toEqual({ name: "", type: undefined, options: 0 });
    expect(columnVersion({ name: 3, type: 4, config: [] })).toEqual({
      name: "",
      type: undefined,
      options: 0,
    });
  });

  it("says what differs between the two versions", () => {
    const ours = { name: "Value", type: "number" as const, options: 0 };
    expect(columnDifferences(ours, { ...ours })).toEqual([]);
    expect(columnDifferences(ours, { ...ours, name: "Contract" })).toEqual(["name"]);
    expect(columnDifferences(ours, { name: "Contract", type: "text", options: 3 })).toEqual([
      "name",
      "type",
      "options",
    ]);
  });

  it("is syncing while the link is not stopped", () => {
    const link = (state: "syncing" | "paused" | "stopped"): TableSync => ({
      link: {
        connection: "c",
        data_source: "d",
        state,
        owner: "u",
        conflicts: 0,
      },
      conflicts: [],
    });
    expect(isSyncing()).toBe(false);
    expect(isSyncing({ link: null, conflicts: [] })).toBe(false);
    expect(isSyncing(link("syncing"))).toBe(true);
    expect(isSyncing(link("paused"))).toBe(true);
    expect(isSyncing(link("stopped"))).toBe(false);
  });
});
