// How long a message body stays fresh (campbooks-cloud#236). A body never
// changes once stored, but whether its remote images show does, with the
// rules. Where the client follows the change feed live (the web), a rule's
// change re-reads the bodies whose images depend on rules, so they stay
// fresh until then. Where it cannot yet (the desktop and mobile shells, until
// their native change source exists: see `followsChangesLive`), those bodies
// alone go stale after a minute and are re-read on focus; a body without
// remote-image state never needs it.
import { holdsImageState } from "~/lib/api";

/** How soon a body whose images depend on rules is re-read without a live feed. */
export const BODY_IMAGES_STALE_MS = 60_000;

/** The body query's `staleTime`, from its data and whether changes come live. */
export function bodyStaleTime(data: unknown, live: boolean): number {
  return !live && holdsImageState(data) ? BODY_IMAGES_STALE_MS : Number.POSITIVE_INFINITY;
}

/** Whether the body query refetches when the window regains focus. */
export function refetchBodyOnFocus(data: unknown, live: boolean): boolean {
  return !live && holdsImageState(data);
}
