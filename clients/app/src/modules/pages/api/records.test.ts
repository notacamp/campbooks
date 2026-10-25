import { describe, expect, it } from "vitest";

import type { PageSummary } from "../types";

import { pageLabel } from "./records";

const page = (id: string, title: string): PageSummary => ({
  id,
  parent: null,
  place: "workspace",
  position: "a0",
  title,
  icon: null,
  has_children: false,
  role: "reader",
  created_by: "0192f1a0-0000-7000-8000-0000000000a1",
  edited_at: "2026-10-12T09:00:00Z",
  trashed_at: null,
  meta_revision: 1,
});

describe("a page mention's label", () => {
  const tree = [page("p1", "Team handbook")];

  it("comes from the tree, with no page body read", () => {
    expect(pageLabel("w1", "p1", tree)).toEqual({
      state: "ready",
      label: "Team handbook",
      href: "/w/w1/pages/p1",
    });
  });

  it("says only that a page missing from the tree cannot be opened", () => {
    expect(pageLabel("w1", "p2", tree)).toEqual({ state: "closed", label: "" });
  });

  it("is loading until the tree is", () => {
    expect(pageLabel("w1", "p1", undefined).state).toBe("loading");
  });
});
