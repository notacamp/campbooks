import { useNavigate } from "@tanstack/react-router";
import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Chip, ChipRow, NavItem } from "~/lib/ui";

import { STREAMS, useStreams, type ListView, type Stream } from "../../api/queries";
import { streamName, type Translate } from "../../model";

export const streamPath = (workspaceId: string, view: ListView) =>
  `/w/${workspaceId}/inbox/${view}`;

/** Unread conversations per stream (0 until the counts load). */
function useUnread(workspaceId: string): Record<Stream, number> {
  const streams = useStreams(workspaceId);
  const unread = { primary: 0, updates: 0, receipts: 0, newsletters: 0 };
  for (const entry of streams.data ?? []) {
    unread[entry.stream] = entry.unread;
  }
  return unread;
}

/** G then 1–4 opens a stream and G then 5 opens Sent, from anywhere in the workspace. */
function useStreamShortcuts(workspaceId: string) {
  const navigate = useNavigate();
  const go = (view: ListView) => () => {
    void navigate({ to: streamPath(workspaceId, view) });
  };
  useShortcut("inbox.stream.primary", go("primary"));
  useShortcut("inbox.stream.updates", go("updates"));
  useShortcut("inbox.stream.receipts", go("receipts"));
  useShortcut("inbox.stream.newsletters", go("newsletters"));
  useShortcut("inbox.sent", go("sent"));
}

export interface StreamNavProps {
  workspaceId: string;
  /** The stream (or Sent) on screen, if any. */
  current?: ListView | undefined;
}

/**
 * The streams under Inbox in the sidebar, with their unread counts and
 * shortcuts (G then 1–4, bound here so they work from every page), and Sent
 * after them, set apart (G then 5).
 */
export function StreamNav({ workspaceId, current }: StreamNavProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const unread = useUnread(workspaceId);
  useStreamShortcuts(workspaceId);
  return (
    <>
      {STREAMS.map((stream) => {
        const href = streamPath(workspaceId, stream);
        return (
          <NavItem
            key={stream}
            sub
            label={streamName(t, stream)}
            count={unread[stream] > 0 ? unread[stream] : undefined}
            shortcut={`inbox.stream.${stream}`}
            current={stream === current}
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
      })}
      <NavItem
        sub
        apart
        label={streamName(t, "sent")}
        shortcut="inbox.sent"
        current={current === "sent"}
        href={streamPath(workspaceId, "sent")}
        onClick={(event: MouseEvent<HTMLElement>) => {
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
            return;
          }
          event.preventDefault();
          void navigate({ to: streamPath(workspaceId, "sent") });
        }}
      />
    </>
  );
}

/** The streams as chips, for phones (where the sidebar is hidden). */
export function StreamChips({ workspaceId, current }: StreamNavProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const unread = useUnread(workspaceId);
  return (
    <ChipRow label={t("streams.label")}>
      {STREAMS.map((stream) => (
        <Chip
          key={stream}
          pressed={stream === current}
          onClick={() => {
            void navigate({ to: streamPath(workspaceId, stream) });
          }}
        >
          {streamName(t, stream)}
          {unread[stream] > 0 ? <span>{unread[stream]}</span> : null}
        </Chip>
      ))}
      <Chip
        pressed={current === "sent"}
        onClick={() => {
          void navigate({ to: streamPath(workspaceId, "sent") });
        }}
      >
        {streamName(t, "sent")}
      </Chip>
    </ChipRow>
  );
}
