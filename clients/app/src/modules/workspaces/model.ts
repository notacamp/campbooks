import type { Accent } from "~/lib/ui";

import type { Kind, Role, Workspace } from "./api/queries";

/** The accent preset a new workspace of this kind gets (the server's default too). */
export const ACCENT_FOR_KIND: Record<Kind, Accent> = {
  business: "business",
  personal: "personal",
};

/** Owners and admins manage members and invitations. */
export function canManage(workspace: Pick<Workspace, "role">): boolean {
  return workspace.role === "owner" || workspace.role === "admin";
}

/**
 * The roles `actor` may give a member who now has `target`: an admin never
 * touches the owner role (the API refuses it); a member manages nobody.
 */
export function assignableRoles(actor: Role | null | undefined, target: Role): Role[] {
  if (actor === "owner") {
    return ["owner", "admin", "member"];
  }
  if (actor === "admin" && target !== "owner") {
    return ["admin", "member"];
  }
  return [];
}

/** Whether `actor` may remove a member who has `target`. */
export function canRemove(actor: Role | null | undefined, target: Role): boolean {
  return actor === "owner" || (actor === "admin" && target !== "owner");
}

/** A loose check before asking the server, which has the final say. */
export function looksLikeEmail(value: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/u.test(value.trim());
}

/** The longest workspace name the API accepts. */
export const MAX_NAME_LENGTH = 100;
