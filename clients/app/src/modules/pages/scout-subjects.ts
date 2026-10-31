import { usePages } from "./api/queries";

/** What a chat with Scout is about, in one line, or why it can't be said. */
interface ChatSubject {
  /** The line, once read. */
  text: string | undefined;
  /** It could not be read: gone, or no longer yours to see. */
  failed: boolean;
}

/**
 * A page as a chat about it names it: its title. Asks for nothing without a
 * page. A page with no title yet reads as an empty line, which the chat says
 * as "Untitled".
 */
export function usePageSubject(workspaceId: string, pageId: string | undefined): ChatSubject {
  const pages = usePages(workspaceId);
  const page = pages.data?.find((candidate) => candidate.id === pageId);
  return {
    text: page?.title,
    failed: pages.isError || (pages.isSuccess && pageId !== undefined && page === undefined),
  };
}

/**
 * Where a page sits, outermost first, the page itself last: "Clients",
 * "Acme", "Kickoff notes". Read from the tree the member can open, so an
 * ancestor they cannot open is not named.
 */
export function usePageTrail(workspaceId: string, pageId: string | undefined): string[] {
  const pages = usePages(workspaceId).data ?? [];
  const trail: string[] = [];
  let at = pages.find((candidate) => candidate.id === pageId);
  // A tree is finite, but never trust it to be acyclic.
  for (let depth = 0; at !== undefined && depth < 32; depth += 1) {
    trail.unshift(at.title);
    const parent = at.parent;
    at =
      parent === null || parent === undefined
        ? undefined
        : pages.find((page) => page.id === parent);
  }
  return trail;
}
