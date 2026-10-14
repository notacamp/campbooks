import {
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  type InfiniteData,
} from "@tanstack/react-query";

import { adminApi, unwrap, type AdminSchema } from "~/lib/api";

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
