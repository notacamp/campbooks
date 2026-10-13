import { infiniteQueryOptions, keepPreviousData, queryOptions } from "@tanstack/react-query";

import { adminApi, unwrap, type AdminSchema } from "~/lib/api";

/** Admin → People and Workspaces (admin API, ADR 0015): metadata only. */
export type AdminUser = AdminSchema<"AdminUser">;
export type AdminUserPage = AdminSchema<"AdminUserPage">;
export type AdminUserDetail = AdminSchema<"AdminUserDetail">;
export type AdminWorkspacePage = AdminSchema<"AdminWorkspacePage">;
export type AdminWorkspaceDetail = AdminSchema<"AdminWorkspaceDetail">;

/** The search text as the API takes it: trimmed, or none. */
const searchText = (q: string) => {
  const text = q.trim();
  return text === "" ? undefined : text;
};

/** A list's query string: the search and the page cursor, each only when set. */
const listQuery = (q: string, after: string | undefined) => {
  const text = searchText(q);
  return {
    ...(text === undefined ? {} : { q: text }),
    ...(after === undefined ? {} : { after }),
  };
};

/** `workspaces.admin_list_users`: people, newest first, a page at a time. */
export const adminUsersQuery = (q: string) =>
  infiniteQueryOptions({
    queryKey: ["admin", "workspaces.admin_list_users", searchText(q) ?? ""] as const,
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/workspaces/users", {
          params: { query: listQuery(q, pageParam) },
          signal,
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page: AdminUserPage) => page.next ?? undefined,
    // A new search keeps the rows on screen until its answer arrives.
    placeholderData: keepPreviousData,
  });

/** `workspaces.admin_get_user`: a person and their workspaces. */
export const adminUserQuery = (userId: string) =>
  queryOptions({
    queryKey: ["admin", "workspaces.admin_get_user", userId] as const,
    queryFn: async ({ signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/workspaces/users/{user_id}", {
          params: { path: { user_id: userId } },
          signal,
        }),
      ),
  });

/** `workspaces.admin_list_workspaces`: workspaces, newest first, a page at a time. */
export const adminWorkspacesQuery = (q: string) =>
  infiniteQueryOptions({
    queryKey: ["admin", "workspaces.admin_list_workspaces", searchText(q) ?? ""] as const,
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/workspaces/workspaces", {
          params: { query: listQuery(q, pageParam) },
          signal,
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page: AdminWorkspacePage) => page.next ?? undefined,
    placeholderData: keepPreviousData,
  });

/** `workspaces.admin_get_workspace`: a workspace, its members and pending invitations. */
export const adminWorkspaceQuery = (workspaceId: string) =>
  queryOptions({
    queryKey: ["admin", "workspaces.admin_get_workspace", workspaceId] as const,
    queryFn: async ({ signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/workspaces/workspaces/{workspace_id}", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ),
  });
