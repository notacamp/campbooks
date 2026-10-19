import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Chip, ChipDivider, ChipRow, NavDivider, NavItem, type IconName } from "~/lib/ui";

import {
  SIDE_PLACES,
  STREAMS,
  useStreams,
  type Drafts,
  type Place,
  type Stream,
  type View,
} from "../../api/queries";
import { placeName, type Translate } from "../../model";

/** `/w/$workspaceId/inbox/$stream`: a stream's list, or a view's. */
export const streamPath = (workspaceId: string, place: Place) => `/w/${workspaceId}/inbox/${place}`;

/** Each view's icon in the sidebar. */
const VIEW_ICONS: Record<View | Drafts, IconName> = {
  sent: "send",
  starred: "pin",
  drafts: "document",
  archive: "archive",
  spam: "warning",
  trash: "trash",
};

/** Unread conversations per stream (0 until the counts load). */
function useUnread(workspaceId: string): Record<Stream, number> {
  const streams = useStreams(workspaceId);
  const unread = { primary: 0, updates: 0, receipts: 0, newsletters: 0 };
  for (const entry of streams.data ?? []) {
    unread[entry.stream] = entry.unread;
  }
  return unread;
}

/**
 * G then 1–4 opens a stream, G then 5–8 Sent, Archive, Spam or Trash, G then 9
 * Pinned and G then 0 Drafts, from anywhere in the workspace. The keys of 5–8
 * stay as they were; the sidebar follows meaning, the keys follow history.
 */
function usePlaceShortcuts(workspaceId: string) {
  const navigate = useNavigate();
  const go = (place: Place) => () => {
    void navigate({ to: streamPath(workspaceId, place) });
  };
  useShortcut("inbox.stream.primary", go("primary"));
  useShortcut("inbox.stream.updates", go("updates"));
  useShortcut("inbox.stream.receipts", go("receipts"));
  useShortcut("inbox.stream.newsletters", go("newsletters"));
  useShortcut("inbox.view.sent", go("sent"));
  useShortcut("inbox.view.archive", go("archive"));
  useShortcut("inbox.view.spam", go("spam"));
  useShortcut("inbox.view.trash", go("trash"));
  useShortcut("inbox.view.starred", go("starred"));
  useShortcut("inbox.view.drafts", go("drafts"));
}

export interface StreamNavProps {
  workspaceId: string;
  /** The stream or view on screen, if any. */
  current?: Place | undefined;
}

/**
 * The streams under Inbox in the sidebar, with their unread counts, then a
 * quiet divider and the places beside them (Sent, Pinned, Drafts, Archive,
 * Spam, Trash; no counts), each with its shortcut (G then 1–9 and 0, bound
 * here so they work from every page).
 */
export function StreamNav({ workspaceId, current }: StreamNavProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const unread = useUnread(workspaceId);
  usePlaceShortcuts(workspaceId);
  const item = (place: Place, shortcut: string, extra: { count?: number; icon?: IconName }) => {
    const href = streamPath(workspaceId, place);
    return (
      <NavItem
        key={place}
        sub
        label={placeName(t, place)}
        count={extra.count}
        icon={extra.icon}
        shortcut={shortcut}
        current={place === current}
        href={href}
        onClick={(event: MouseEvent<HTMLElement>) => {
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
            return;
          }
          event.preventDefault();
          void navigate({ to: href });
        }}
      />
    );
  };
  return (
    <>
      {STREAMS.map((stream) =>
        item(stream, `inbox.stream.${stream}`, {
          ...(unread[stream] > 0 ? { count: unread[stream] } : {}),
        }),
      )}
      <NavDivider />
      {SIDE_PLACES.map((place) => item(place, `inbox.view.${place}`, { icon: VIEW_ICONS[place] }))}
    </>
  );
}

/**
 * The streams, then the other places behind a divider, as chips for phones
 * (where the sidebar is hidden): one row that scrolls sideways on its own,
 * and brings the chip of the place you are in into view.
 */
export function StreamChips({ workspaceId, current }: StreamNavProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const unread = useUnread(workspaceId);
  const active = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
    // Not every environment lays out or animates (tests, old webviews).
    const reduced =
      typeof globalThis.matchMedia === "function" &&
      globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const chip: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
      active.current;
    chip?.scrollIntoView?.({
      inline: "nearest",
      block: "nearest",
      behavior: reduced ? "auto" : "smooth",
    });
  }, [current]);
  const chip = (place: Place, count = 0) => (
    <Chip
      key={place}
      ref={place === current ? active : undefined}
      pressed={place === current}
      onClick={() => {
        void navigate({ to: streamPath(workspaceId, place) });
      }}
    >
      {placeName(t, place)}
      {count > 0 ? <span>{count}</span> : null}
    </Chip>
  );
  return (
    <ChipRow label={t("streams.label")}>
      {STREAMS.map((stream) => chip(stream, unread[stream]))}
      <ChipDivider />
      {SIDE_PLACES.map((place) => chip(place))}
    </ChipRow>
  );
}
