import { describe, expect, it } from "vitest";

import { assignableRoles, canManage, canRemove, looksLikeEmail } from "./model";

describe("workspace permissions", () => {
  it("lets owners and admins manage", () => {
    expect(canManage({ role: "owner" })).toBe(true);
    expect(canManage({ role: "admin" })).toBe(true);
    expect(canManage({ role: "member" })).toBe(false);
    expect(canManage({ role: null })).toBe(false);
  });

  it("keeps admins away from the owner role", () => {
    expect(assignableRoles("owner", "owner")).toEqual(["owner", "admin", "member"]);
    expect(assignableRoles("admin", "member")).toEqual(["admin", "member"]);
    expect(assignableRoles("admin", "owner")).toEqual([]);
    expect(assignableRoles("member", "member")).toEqual([]);
    expect(canRemove("owner", "owner")).toBe(true);
    expect(canRemove("admin", "member")).toBe(true);
    expect(canRemove("admin", "owner")).toBe(false);
    expect(canRemove(undefined, "member")).toBe(false);
  });

  it("checks an email loosely", () => {
    expect(looksLikeEmail(" sofia@example.com ")).toBe(true);
    expect(looksLikeEmail("sofia@")).toBe(false);
    expect(looksLikeEmail("sofia example.com")).toBe(false);
  });
});
