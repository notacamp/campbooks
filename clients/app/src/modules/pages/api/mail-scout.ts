import { useQuery } from "@tanstack/react-query";

import { api, errorStatus, operationKey, unwrap } from "~/lib/api";

import type { AttachedFile, PageView } from "../types";

import { usePages } from "./queries";

const SHARING = "pages.get_sharing";

const sharingKey = (workspaceId: string, pageId: string) =>
  operationKey(SHARING, { workspace_id: workspaceId, page_id: pageId });

/** `pages.get_sharing`: who really reads a page, and whether Notion gets what is added. */
export function usePageSharing(workspaceId: string, pageId: string | undefined) {
  return useQuery({
    queryKey: sharingKey(workspaceId, pageId ?? ""),
    enabled: pageId !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/pages/{page_id}/sharing", {
          params: { path: { workspace_id: workspaceId, page_id: pageId ?? "" } },
          signal,
        }),
      ),
    // Who reads a page changes rarely; the choice always asks again when it opens.
    staleTime: 0,
    retry: (count, error) => errorStatus(error) !== 404 && count < 2,
  });
}

/**
 * Whether the server offers mail and Scout in pages. It does not say so in
 * one place (the pages list carries only `tables`): while the switch is off
 * every one of its routes is a 404, so the answer of `pages.get_sharing` for
 * a page the caller can open is the test. `probe` is such a page: the page
 * on screen, or the first one of the tree. `undefined` while that is not
 * known yet (and with no page to ask about), `false` once the server says no.
 */
export function useMailScoutAvailable(workspaceId: string, probe?: string): boolean | undefined {
  const list = usePages(workspaceId);
  const first = list.data?.find((page) => page.kind === "page")?.id;
  const page = probe ?? first;
  const sharing = useQuery({
    queryKey: [...sharingKey(workspaceId, page ?? ""), "available"],
    enabled: page !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/pages/{page_id}/sharing", {
          params: { path: { workspace_id: workspaceId, page_id: page ?? "" } },
          signal,
        }),
      ),
    retry: false,
    staleTime: 5 * 60_000,
    refetchOnWindowFocus: false,
  });
  if (sharing.isSuccess) {
    return true;
  }
  return sharing.isError && errorStatus(sharing.error) === 404 ? false : undefined;
}

/** `pages.get`, once: a page with its body, as the dialog appends to it. */
export async function readPage(workspaceId: string, pageId: string): Promise<PageView> {
  return unwrap(
    await api.GET("/api/v1/w/{workspace_id}/pages/{page_id}", {
      params: { path: { workspace_id: workspaceId, page_id: pageId } },
    }),
  );
}

/** The calls Save to page makes besides `pages.sync`. */
export function mailScoutCalls(workspaceId: string) {
  const path = { workspace_id: workspaceId };
  return {
    /** `pages.attach_from_message`: all of the message's attachments asked for, or none. */
    attach: async (
      pageId: string,
      conversation: string,
      message: string,
      attachments: readonly string[],
    ): Promise<AttachedFile[]> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/files/from-message", {
          params: { path: { ...path, page_id: pageId } },
          body: { conversation, message, attachments: [...attachments] },
        }),
      ).files,
    /** `pages.detach_files`: takes back files that came from a message; the page keeps any a block names. */
    detach: async (pageId: string, blobs: readonly string[]) => {
      if (blobs.length === 0) {
        return;
      }
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/files/detach", {
          params: { path: { ...path, page_id: pageId } },
          body: { files: [...blobs] },
        }),
      );
    },
    /** `pages.trash`: a page made for this alone goes back with its Undo. */
    trash: async (pageId: string): Promise<void> => {
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/trash", {
          params: { path: { ...path, page_id: pageId } },
        }),
      );
    },
  };
}
