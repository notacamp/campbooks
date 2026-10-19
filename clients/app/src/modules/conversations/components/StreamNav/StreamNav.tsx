import { useNavigate } from "@tanstack/react-router";
import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Chip, ChipDivider, ChipRow, NavDivider, NavItem, type IconName } from "~/lib/ui";

import { STREAMS, VIEWS, useStreams, type Place, type Stream, type View } from "../../api/queries";
import { placeName, type Translate } from "../../model";

/** `/w/$workspaceId/inbox/$stream`: a stream's list, or a view's. */
export const streamPath = (workspaceId: string, place: Place) => `/w/${workspaceId}/inbox/${place}`;

/** Each view's icon in the sidebar. */
const VIEW_ICONS: Record<View, IconName> = {
  sent: "send",
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

/** G then 1–4 opens a stream and G then 5–8 a view, from anywhere in the workspace. */
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
}

export interface StreamNavProps {
  workspaceId: string;
  /** The stream or view on screen, if any. */
  current?: Place | undefined;
}

/**
 * The streams under Inbox in the sidebar, with their unread counts, then a
 * quiet divider and the views beside them (Sent, Archive, Spam, Trash; no
 * counts), each with its shortcut (G then 1–8, bound here so they work from
 * every page).
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
      {VIEWS.map((view) => item(view, `inbox.view.${view}`, { icon: VIEW_ICONS[view] }))}
    </>
  );
}

/**
 * The streams, then the views behind a divider, as chips for phones (where
 * the sidebar is hidden): one row that scrolls sideways on its own.
 */
export function StreamChips({ workspaceId, current }: StreamNavProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const unread = useUnread(workspaceId);
  const chip = (place: Place, count = 0) => (
    <Chip
      key={place}
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
      {VIEWS.map((view) => chip(view))}
    </ChipRow>
  );
}
