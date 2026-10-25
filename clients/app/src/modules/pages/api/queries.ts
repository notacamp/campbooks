import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";

import { api, errorStatus, operationKey, unwrap, unwrapEmpty } from "~/lib/api";

import type {
  Backlinks,
  GrantView,
  NewPage,
  PageList,
  PageMoveBody,
  PagePlace,
  PageRole,
  PageSummary,
  PageView,
  SyncResponse,
  UploadedFile,
} from "../types";

// Keyed by operationId, so the change feed refetches them when a
// `pages.page` changes.
const LIST = "pages.list";
const GET = "pages.get";
const BACKLINKS = "pages.backlinks";

const keys = {
  list: (workspaceId: string) => operationKey(LIST, { workspace_id: workspaceId }),
  trash: (workspaceId: string) =>
    operationKey(LIST, { workspace_id: workspaceId, trashed: "true" }),
  page: (workspaceId: string, id: string) =>
    operationKey(GET, { workspace_id: workspaceId, page_id: id }),
  backlinks: (workspaceId: string, type: string, id: string) =>
    operationKey(BACKLINKS, { workspace_id: workspaceId, type, id }),
};

/** A page the server answers 404 to is one the caller cannot open (or no pages at all while off). */
const retryUnlessMissing = (count: number, error: unknown) =>
  errorStatus(error) !== 404 && count < 2;

/** `pages.list`: the pages the caller may open, flat. */
export function usePages(workspaceId: string) {
  return useQuery({
    queryKey: keys.list(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/pages", {
          params: { path: { workspace_id: workspaceId }, query: {} },
          signal,
        }),
      ),
    select: (list: PageList) => list.pages,
    retry: retryUnlessMissing,
  });
}

/** `pages.list?trashed=true`: what was deleted in the last 30 days. */
export function useTrash(workspaceId: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.trash(workspaceId),
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/pages", {
          params: { path: { workspace_id: workspaceId }, query: { trashed: true } },
          signal,
        }),
      ),
    select: (list: PageList) => list.pages,
  });
}

/**
 * Whether the Pages place is on: `true` once the list answers, `false` when
 * the server says there are no pages here (the module is switched off),
 * `undefined` while that is not known yet.
 */
export function usePagesAvailable(workspaceId: string): boolean | undefined {
  const pages = usePages(workspaceId);
  if (pages.isSuccess) {
    return true;
  }
  return pages.isError && errorStatus(pages.error) === 404 ? false : undefined;
}

/** `pages.get`: one page with its body; refetched when the change feed says it changed. */
export function usePage(workspaceId: string, id: string | undefined) {
  return useQuery({
    queryKey: keys.page(workspaceId, id ?? ""),
    enabled: id !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/pages/{page_id}", {
          params: { path: { workspace_id: workspaceId, page_id: id ?? "" } },
          signal,
        }),
      ),
    retry: retryUnlessMissing,
    // The editor holds the live document; only the change feed asks again.
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: false,
  });
}

/** `pages.backlinks`: the pages the caller may open that link to a record. */
export function useBacklinks(workspaceId: string, type: string, id: string) {
  return useQuery({
    queryKey: keys.backlinks(workspaceId, type, id),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/pages/backlinks", {
          params: { path: { workspace_id: workspaceId }, query: { type, id } },
          signal,
        }),
      ),
    select: (links: Backlinks) => links.pages,
    retry: retryUnlessMissing,
  });
}

/** Refetch the tree, the trash and the pages on screen. */
export function useRefreshPages(workspaceId: string) {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: keys.list(workspaceId) });
    void queryClient.invalidateQueries({ queryKey: keys.trash(workspaceId) });
  };
}

/** `sync`, as the editor's synchroniser calls it. */
export interface SyncBody { state_vector: string; update?: string }

/** The calls a page takes. Each answers what the server made of it. */
export function usePageCalls(workspaceId: string) {
  return useMemo(() => pageCalls(workspaceId), [workspaceId]);
}

function pageCalls(workspaceId: string) {
  const path = { workspace_id: workspaceId };
  const at = (page: string) => ({ path: { ...path, page_id: page } });
  const grantAt = (page: string, member: string) => ({
    path: { ...path, page_id: page, member_id: member },
  });
  return {
    create: async (body: NewPage): Promise<PageView> =>
      unwrap(await api.POST("/api/v1/w/{workspace_id}/pages", { params: { path }, body })),
    update: async (
      page: PageSummary,
      change: { title: string; icon: string | null },
    ): Promise<PageSummary> =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/pages/{page_id}", {
          params: at(page.id),
          body: { meta_revision: page.meta_revision, ...change },
        }),
      ),
    move: async (
      page: PageSummary,
      to: { parent: string | null; place: PagePlace; position: string },
    ): Promise<PageSummary> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/move", {
          params: at(page.id),
          body: { meta_revision: page.meta_revision, ...to } satisfies PageMoveBody,
        }),
      ),
    trash: async (page: PageSummary): Promise<PageSummary> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/trash", {
          params: at(page.id),
        }),
      ),
    restore: async (page: PageSummary): Promise<PageSummary> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/restore", {
          params: at(page.id),
        }),
      ),
    purge: async (page: PageSummary): Promise<void> => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/pages/{page_id}", { params: at(page.id) }),
      );
    },
    share: async (page: string, member: string, role: PageRole): Promise<GrantView> =>
      unwrap(
        await api.PUT("/api/v1/w/{workspace_id}/pages/{page_id}/grants/{member_id}", {
          params: grantAt(page, member),
          body: { role },
        }),
      ),
    unshare: async (page: string, member: string): Promise<void> => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/pages/{page_id}/grants/{member_id}", {
          params: grantAt(page, member),
        }),
      );
    },
    sync: async (page: string, body: SyncBody): Promise<SyncResponse> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/sync", {
          params: at(page),
          body,
        }),
      ),
    upload: async (page: string, file: File): Promise<UploadedFile> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/files", {
          params: at(page),
          body: file as unknown as string,
          bodySerializer: (body: unknown) => body as BodyInit,
          headers: { "Content-Type": file.type === "" ? "application/octet-stream" : file.type },
        }),
      ),
  };
}
