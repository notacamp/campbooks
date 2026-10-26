import type { Member } from "~/modules/workspaces";

/** What a cell needs to know about where it is shown. */
export interface CellContext {
  workspaceId: string;
  /** The viewer's language, for numbers and dates. */
  locale: string;
  members: readonly Member[];
  /** Open an address in the app without a reload. */
  open: (href: string) => void;
}

/** A person's name for a member id, or nothing when they are not in the workspace. */
export function memberName(members: readonly Member[], id: string): string | undefined {
  const member = members.find((candidate) => candidate.user_id === id);
  return member === undefined ? undefined : (member.display_name ?? member.email ?? id);
}
