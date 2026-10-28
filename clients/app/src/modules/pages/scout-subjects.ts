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
