import { useQuery } from "@tanstack/react-query";

import { api, errorStatus, unwrap } from "~/lib/api";

import type { MentionType, PageSummary } from "../types";

import { usePages } from "./queries";

/** What a mention shows: the record's own current name, or that it cannot be opened. */
export interface RecordLabel {
  state: "ready" | "loading" | "closed";
  label: string;
  /** Where it opens in the app. */
  href?: string | undefined;
}

const NOT_OPEN: RecordLabel = { state: "closed", label: "" };

/**
 * Reads a record through its own module's read operation with the viewer's
 * token (never a copy kept in the page): a 404 is a record the viewer
 * cannot open, and is shown as nothing more than that.
 */
async function read(
  workspaceId: string,
  type: MentionType,
  id: string,
  signal: AbortSignal,
): Promise<Omit<RecordLabel, "state"> & { found: boolean }> {
  const workspace = { workspace_id: workspaceId };
  switch (type) {
    case "conversations.conversation": {
      const detail = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/conversations/{conversation_id}", {
          params: { path: { ...workspace, conversation_id: id } },
          signal,
        }),
      );
      return {
        found: true,
        label: detail.conversation.subject,
        href: `/w/${workspaceId}/inbox/${detail.conversation.stream}/${id}`,
      };
    }
    case "tasks.task": {
      const task = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/tasks/{task_id}", {
          params: { path: { ...workspace, task_id: id }, query: {} },
          signal,
        }),
      );
      return { found: true, label: task.task.title, href: `/w/${workspaceId}/tasks/${id}` };
    }
    case "schedule.event": {
      const event = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/schedule/events/{event_id}", {
          params: { path: { ...workspace, event_id: id } },
          signal,
        }),
      );
      return { found: true, label: event.title, href: `/w/${workspaceId}/schedule?event=${id}` };
    }
    case "workspaces.member": {
      const members = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/members", {
          params: { path: workspace },
          signal,
        }),
      ).members;
      const member = members.find((candidate) => candidate.user_id === id);
      return member === undefined
        ? { found: false, label: "" }
        : { found: true, label: member.display_name ?? member.email ?? id };
    }
    case "person": {
      // The name the viewer's own mail gives this address, else the address.
      const people = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/correspondents", {
          params: { path: workspace, query: { q: id, limit: 5 } },
          signal,
        }),
      ).correspondents;
      const match = people.find((person) => person.address.toLowerCase() === id);
      return {
        found: true,
        label: match?.name ?? id,
        href: `/w/${workspaceId}/search?q=${encodeURIComponent(`from:${id}`)}`,
      };
    }
    case "pages.page": {
      // Never read here: `useRecordLabel` takes a page's name from the tree
      // (metadata only), so a page of mentions downloads no page bodies.
      return { found: false, label: "" };
    }
  }
}

/**
 * A page mention's label from the tree the viewer may open (`pages.list`,
 * titles and ids only): a page that is not in it is one they cannot open.
 */
export function pageLabel(
  workspaceId: string,
  id: string,
  pages?: readonly PageSummary[],
): RecordLabel {
  if (pages === undefined) {
    return { state: "loading", label: "" };
  }
  const page = pages.find((candidate) => candidate.id === id);
  return page === undefined
    ? NOT_OPEN
    : { state: "ready", label: page.title, href: `/w/${workspaceId}/pages/${id}` };
}

/** The mention's label for the viewer, read when it is first shown and kept fresh by the cache. */
export function useRecordLabel(workspaceId: string, type: MentionType, id: string): RecordLabel {
  const tree = usePages(workspaceId);
  const query = useQuery({
    queryKey: ["pages", "mention", workspaceId, type, id],
    queryFn: ({ signal }) => read(workspaceId, type, id, signal),
    enabled: type !== "pages.page",
    staleTime: 60_000,
    retry: (count, error) => errorStatus(error) !== 404 && errorStatus(error) !== 403 && count < 2,
  });
  if (type === "pages.page") {
    return tree.isError ? NOT_OPEN : pageLabel(workspaceId, id, tree.data);
  }
  if (query.isPending) {
    return { state: "loading", label: "" };
  }
  if (query.isError || !query.data.found) {
    return NOT_OPEN;
  }
  const { label, href } = query.data;
  return { state: "ready", label, href };
}
