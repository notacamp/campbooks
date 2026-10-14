import {
  infiniteQueryOptions,
  keepPreviousData,
  queryOptions,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  type InfiniteData,
} from "@tanstack/react-query";

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

/** Admin → Signups. */
export type SignupRequest = AdminSchema<"SignupRequestView">;
export type SignupRequestStatus = AdminSchema<"RequestStatus">;
export type SignupCode = AdminSchema<"SignupCodeView">;
export type MintedSignupCode = AdminSchema<"MintedSignupCode">;
export type MintSignupCode = AdminSchema<"MintSignupCode">;

const REQUESTS_KEY = ["admin", "workspaces.admin_list_signup_requests"] as const;
const CODES_KEY = ["admin", "workspaces.admin_list_signup_codes"] as const;

/** `workspaces.admin_list_signup_requests`, a page at a time, pending first. */
export function useSignupRequests(status: SignupRequestStatus | "all") {
  return useInfiniteQuery({
    queryKey: [...REQUESTS_KEY, status],
    initialPageParam: undefined as string | undefined,
    retry: false,
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/workspaces/signup-requests", {
          params: {
            query: {
              ...(status === "all" ? {} : { status }),
              ...(pageParam === undefined ? {} : { after: pageParam }),
            },
          },
          signal,
        }),
      ),
    getNextPageParam: (page) => page.next ?? undefined,
  });
}

/** `workspaces.admin_list_signup_codes`, a page at a time, newest first. */
export function useSignupCodes() {
  return useInfiniteQuery({
    queryKey: CODES_KEY,
    initialPageParam: undefined as string | undefined,
    retry: false,
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/workspaces/signup-codes", {
          params: { query: pageParam === undefined ? {} : { after: pageParam } },
          signal,
        }),
      ),
    getNextPageParam: (page) => page.next ?? undefined,
  });
}

/** `workspaces.admin_approve_signup_request` / `…_reject_…`. */
export function useDecideSignupRequest() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      userId,
      decision,
      reason,
    }: {
      userId: string;
      decision: "approve" | "reject";
      reason?: string | undefined;
    }) => {
      const params = { path: { user_id: userId } };
      return decision === "approve"
        ? unwrap(
            await adminApi.POST("/admin/v1/workspaces/signup-requests/{user_id}/approve", {
              params,
            }),
          )
        : unwrap(
            await adminApi.POST("/admin/v1/workspaces/signup-requests/{user_id}/reject", {
              params,
              body: reason === undefined || reason.trim() === "" ? {} : { reason: reason.trim() },
            }),
          );
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: REQUESTS_KEY });
    },
  });
}

/** `workspaces.admin_mint_signup_code`. The code is in the answer, once. */
export function useMintSignupCode() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: MintSignupCode) =>
      unwrap(await adminApi.POST("/admin/v1/workspaces/signup-codes", { body })),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: CODES_KEY });
    },
  });
}

/** `workspaces.admin_revoke_signup_code`. */
export function useRevokeSignupCode() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (codeId: string) =>
      unwrap(
        await adminApi.POST("/admin/v1/workspaces/signup-codes/{code_id}/revoke", {
          params: { path: { code_id: codeId } },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: CODES_KEY });
    },
  });
}

/** Every row of the pages loaded so far. */
export function allPages<TPage, TRow>(
  data: InfiniteData<TPage> | undefined,
  rows: (page: TPage) => TRow[],
): TRow[] {
  return data === undefined ? [] : data.pages.flatMap((page) => rows(page));
}
