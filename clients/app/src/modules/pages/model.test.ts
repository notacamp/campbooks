import { describe, expect, it } from "vitest";

import { buildSections, dropOn, endOf, isWithin, rowsOf, zoneOf } from "./model";
import type { PageSummary } from "./types";

const ME = "user-ana";

function page(id: string, overrides: Partial<PageSummary> = {}): PageSummary {
  return {
    id,
    parent: null,
    place: "private",
    position: "a0",
    title: id,
    icon: null,
    has_children: false,
    role: "manager",
    created_by: ME,
    edited_at: "2026-10-25T10:00:00Z",
    trashed_at: null,
    meta_revision: 1,
    kind: "page",
    ...overrides,
  };
}

describe("buildSections", () => {
  it("puts top-level pages in Workspace or Private, sorted by position", () => {
    const sections = buildSections(
      [
        page("b", { place: "workspace", position: "a1" }),
        page("a", { place: "workspace", position: "a0" }),
        page("mine"),
        page("sub", { parent: "a", place: "workspace" }),
      ],
      ME,
    );
    expect(sections.workspace.map((node) => node.page.id)).toEqual(["a", "b"]);
    expect(sections.workspace[0]?.children.map((node) => node.page.id)).toEqual(["sub"]);
    expect(sections.private.map((node) => node.page.id)).toEqual(["mine"]);
    expect(sections.shared).toEqual([]);
  });

  it("shows another member's private page, and a page under one the caller cannot open, as shared", () => {
    const sections = buildSections(
      [
        page("theirs", { created_by: "user-rui", role: "reader" }),
        page("leaf", { parent: "hidden-parent", created_by: "user-rui", role: "writer" }),
      ],
      ME,
    );
    expect(sections.shared.map((node) => node.page.id).toSorted()).toEqual(["leaf", "theirs"]);
    expect(sections.private).toEqual([]);
  });
});

describe("rowsOf", () => {
  it("lists a page's sub-pages only while it is open", () => {
    const sections = buildSections(
      [
        page("top", { place: "workspace", has_children: true }),
        page("child", { parent: "top", place: "workspace" }),
      ],
      ME,
    );
    expect(rowsOf(sections.workspace, new Set()).map((row) => [row.page.id, row.expanded])).toEqual(
      [["top", false]],
    );
    expect(
      rowsOf(sections.workspace, new Set(["top"])).map((row) => [row.page.id, row.depth]),
    ).toEqual([
      ["top", 0],
      ["child", 1],
    ]);
  });
});

describe("isWithin", () => {
  it("knows a page is within itself and its descendants, and not within its siblings", () => {
    const sections = buildSections(
      [
        page("top"),
        page("child", { parent: "top" }),
        page("grandchild", { parent: "child" }),
        page("other", { position: "a1" }),
      ],
      ME,
    );
    expect(isWithin(sections, "top", "top")).toBe(true);
    expect(isWithin(sections, "top", "grandchild")).toBe(true);
    expect(isWithin(sections, "child", "top")).toBe(false);
    expect(isWithin(sections, "top", "other")).toBe(false);
    expect(isWithin(sections, "top", null)).toBe(false);
  });
});

describe("dropOn", () => {
  const a = page("a", { position: "a0" });
  const b = page("b", { position: "a1" });
  const c = page("c", { position: "a2" });

  it("puts a page before, between and after its neighbours", () => {
    expect(dropOn(b, "before", [a, b, c], [], "z").position < b.position).toBe(true);
    expect(dropOn(b, "before", [a, b, c], [], "z").position > a.position).toBe(true);
    expect(dropOn(b, "after", [a, b, c], [], "z").position > b.position).toBe(true);
    expect(dropOn(b, "after", [a, b, c], [], "z").position < c.position).toBe(true);
    expect(dropOn(c, "after", [a, b, c], [], "z").position > c.position).toBe(true);
  });

  it("ignores the dragged page among its own neighbours", () => {
    // Dragging b to just after a: between a and c, b itself not counted.
    const moved = dropOn(a, "after", [a, b, c], [], "b");
    expect(moved.position > a.position).toBe(true);
    expect(moved.position < c.position).toBe(true);
  });

  it("nests inside after the last child, in the target's place", () => {
    const child = page("child", { parent: "b", position: "a0" });
    const drop = dropOn(b, "inside", [a, b, c], [child], "z");
    expect(drop.parent).toBe("b");
    expect(drop.position > child.position).toBe(true);
    expect(dropOn(b, "inside", [a, b, c], [], "z").position).toBe("a0");
  });

  it("keeps the target's parent and place for before and after", () => {
    const sub = page("sub", { parent: "top", place: "workspace" });
    const drop = dropOn(sub, "after", [sub], [], "z");
    expect(drop.parent).toBe("top");
    expect(drop.place).toBe("workspace");
  });
});

describe("zoneOf and endOf", () => {
  it("splits a row into top quarter, middle and bottom quarter", () => {
    expect(zoneOf(2, 40)).toBe("before");
    expect(zoneOf(20, 40)).toBe("inside");
    expect(zoneOf(38, 40)).toBe("after");
    expect(zoneOf(5, 0)).toBe("inside");
  });

  it("makes a key after the last sibling", () => {
    expect(endOf([])).toBe("a0");
    expect(endOf([page("a", { position: "a0" }), page("b", { position: "a1" })]) > "a1").toBe(true);
  });
});
