import { queryOptions, useMutation, useQueries, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";

import { adminApi, unwrap, type AdminSchema } from "~/lib/api";

/** An identity account as an administrator sees it (admin API, ADR 0015). */
export type AdminAccount = AdminSchema<"AdminAccount">;

const accountKey = (accountId: string) =>
  ["admin", "identity.admin_get_account", accountId] as const;

const path = (accountId: string) => ({ path: { account_id: accountId } });

/** `identity.admin_get_account`. */
export const adminAccountQuery = (accountId: string) =>
  queryOptions({
    queryKey: accountKey(accountId),
    queryFn: async ({ signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/identity/accounts/{account_id}", {
          params: path(accountId),
          signal,
        }),
      ),
  });

const standingsKey = ["admin", "identity.admin_list_accounts"] as const;

/** `identity.admin_list_accounts`: the standing of `accountIds` (at most 200). */
const adminAccountStandingsQuery = (accountIds: readonly string[]) =>
  queryOptions({
    queryKey: [...standingsKey, accountIds] as const,
    queryFn: async ({ signal }) =>
      unwrap(
        await adminApi.GET("/admin/v1/identity/accounts", {
          params: { query: { ids: accountIds.join(",") } },
          signal,
        }),
      ),
  });

/** Accounts asked about per request: a page of the People list. */
const STANDINGS_CHUNK = 50;

/**
 * The ids among `accountIds` whose accounts are suspended, asked in chunks
 * of a page so the pages already shown stay cached as more load. Empty
 * until known, and when the read fails (the list is still worth showing).
 */
export function useSuspendedAccounts(accountIds: readonly string[]): ReadonlySet<string> {
  const chunks: string[][] = [];
  for (let start = 0; start < accountIds.length; start += STANDINGS_CHUNK) {
    chunks.push(accountIds.slice(start, start + STANDINGS_CHUNK));
  }
  const results = useQueries({
    queries: chunks.map((chunk) => adminAccountStandingsQuery(chunk)),
  });
  const suspended = results
    .flatMap((result) => result.data?.accounts ?? [])
    .filter((account) => account.suspended_at !== null && account.suspended_at !== undefined)
    .map((account) => account.id);
  const key = suspended.join(",");
  return useMemo(() => new Set(key === "" ? [] : key.split(",")), [key]);
}

/**
 * `identity.admin_suspend_account` (`true`) or
 * `identity.admin_unsuspend_account` (`false`). Both answer the account,
 * which replaces the cached one.
 */
export function useSuspendAccount(accountId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (suspend: boolean) =>
      unwrap(
        suspend
          ? await adminApi.POST("/admin/v1/identity/accounts/{account_id}/suspend", {
              params: path(accountId),
            })
          : await adminApi.POST("/admin/v1/identity/accounts/{account_id}/unsuspend", {
              params: path(accountId),
            }),
      ),
    onSuccess: async (account) => {
      queryClient.setQueryData(accountKey(accountId), account);
      await queryClient.invalidateQueries({ queryKey: standingsKey });
    },
  });
}
