// How long a message body stays fresh, and when its remote images may load
// (campbooks-cloud#236). A body never changes once stored, but whether its
// remote images show does, with the rules.
//
// Where the client follows the change feed live (the web), every change to
// the rules drops the bodies a rule trusted and re-reads the others whose
// images depend on rules (`refreshBodiesAfterRules`), so they stay fresh
// until then. Where it cannot yet (the desktop and mobile shells, until their
// native change source exists: see `followsChangesLive`), a rule may change on
// another device without this one knowing. There, a body a rule trusted is
// read again every time it is shown, every minute while it is open and on
// focus, and the reader holds its images back until the read made as it
// opened answers ({@link remoteImagesAllowed}); a body whose images are held
// or offered goes stale after a minute and is read again on focus (it fails
// safe); a body without remote-image state never needs it.
import { useState } from "react";

import { followsChangesLive, holdsImageState, trustsImages } from "~/lib/api";

/**
 * How soon, without a live feed, a body whose images are held or offered goes
 * stale, and how often an open trusted one is read again.
 */
export const BODY_IMAGES_STALE_MS = 60_000;

/** The body query's `staleTime`, from its data and whether changes come live. */
export function bodyStaleTime(data: unknown, live: boolean): number {
  if (live || !holdsImageState(data)) {
    return Number.POSITIVE_INFINITY;
  }
  return trustsImages(data) ? 0 : BODY_IMAGES_STALE_MS;
}

/** The body query's `refetchInterval`: an open trusted body, a minute, without a live feed. */
export function bodyRefetchInterval(data: unknown, live: boolean): number | false {
  return !live && trustsImages(data) ? BODY_IMAGES_STALE_MS : false;
}

/** Whether the body query refetches when the window regains focus. */
export function refetchBodyOnFocus(data: unknown, live: boolean): boolean {
  return !live && holdsImageState(data);
}

/** What {@link remoteImagesAllowed} reads of the body query. */
interface BodyRead {
  data?: { images_shown?: boolean; images_trusted?: boolean } | undefined;
  /** When the data was last read (0 before the first read). */
  dataUpdatedAt: number;
}

/** Where a body is shown, for {@link remoteImagesAllowed}. */
interface ImageContext {
  /** The client follows the change feed live (`followsChangesLive`). */
  live: boolean;
  /** The member asked for this message's remote images. */
  showImages: boolean;
  /** When the body was last read as the message opened; unset while folded. */
  openedAt: number | undefined;
}

/**
 * Whether the reader lets the body's remote images load. Only when the
 * server showed them (`images_shown`). Without a live feed, a body shown
 * because a rule trusts its sender must also have been read since the
 * message opened: a body from the cache may carry a trust removed elsewhere
 * since, so its images stay held back until the fresh read says the rule
 * still holds. The member's own "Show images" needs no rule.
 */
export function remoteImagesAllowed(body: BodyRead, context: ImageContext): boolean {
  if (body.data?.images_shown !== true) {
    return false;
  }
  if (context.live || context.showImages || body.data.images_trusted !== true) {
    return true;
  }
  return context.openedAt !== undefined && body.dataUpdatedAt > context.openedAt;
}

/**
 * {@link remoteImagesAllowed} for one message of the reader, remembering
 * when its body was last read as it opened (forgotten when it folds).
 */
export function useRemoteImagesAllowed(
  body: BodyRead,
  open: boolean,
  showImages: boolean,
): boolean {
  const [openedAt, setOpenedAt] = useState<number | undefined>(
    open ? body.dataUpdatedAt : undefined,
  );
  // Adjusted while rendering, so the first render of an opened message
  // already knows it.
  if (open && openedAt === undefined) {
    setOpenedAt(body.dataUpdatedAt);
  } else if (!open && openedAt !== undefined) {
    setOpenedAt(undefined);
  }
  const since = openedAt ?? body.dataUpdatedAt;
  return remoteImagesAllowed(body, {
    live: followsChangesLive(),
    showImages,
    openedAt: open ? since : undefined,
  });
}
