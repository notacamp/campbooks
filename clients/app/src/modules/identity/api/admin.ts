import { queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";

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
    onSuccess: (account) => {
      queryClient.setQueryData(accountKey(accountId), account);
    },
  });
}
