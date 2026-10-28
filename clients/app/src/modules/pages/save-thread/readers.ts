import type { PageSharing } from "../types";

/** Who will read what is added to a page, without the member themself. */
export interface Readers {
  /** Every member of the workspace. */
  everyone: boolean;
  /** The teammates named, in the workspace's own words for them. */
  names: string[];
  /** How many more read it than are named. */
  others: number;
}

/** A member of the workspace, as far as naming a reader needs. */
export interface Named {
  user_id: string;
  you?: boolean | undefined;
  display_name?: string | null | undefined;
  email?: string | null | undefined;
}

/** The most readers named; the rest are counted. */
export const NAMED_MAX = 5;

/**
 * The readers of a page from `pages.get_sharing`: the caller left out (they
 * know), the others named by the workspace's member list, and anyone it
 * cannot name counted instead. A name is never an id.
 */
export function readersOf(sharing: PageSharing, members: readonly Named[]): Readers {
  if (sharing.everyone) {
    return { everyone: true, names: [], others: 0 };
  }
  const names: string[] = [];
  let others = 0;
  for (const reader of sharing.readers) {
    const member = members.find((candidate) => candidate.user_id === reader);
    if (member?.you === true) {
      continue;
    }
    const name = member?.display_name ?? member?.email ?? undefined;
    if (name === undefined || name === "" || names.length >= NAMED_MAX) {
      others += 1;
    } else {
      names.push(name);
    }
  }
  return { everyone: false, names, others };
}

/** Words for a list: the pieces and the word for "and" in the member's language. */
export function joinNames(pieces: readonly string[], and: string): string {
  if (pieces.length <= 1) {
    return pieces.join("");
  }
  return `${pieces.slice(0, -1).join(", ")} ${and} ${pieces.at(-1) ?? ""}`;
}
