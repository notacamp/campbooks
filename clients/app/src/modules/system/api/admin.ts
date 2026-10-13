import { queryOptions } from "@tanstack/react-query";

import { adminApi, unwrap, type AdminSchema } from "~/lib/api";

/** What `GET /admin/v1/system/overview` answers (admin API, ADR 0015). */
export type AdminOverview = AdminSchema<"AdminOverview">;
export type AiRoleStatus = AdminSchema<"AiRoleStatus">;

/**
 * The admin system overview. It also tells the app whether the person is an
 * administrator at all (404 when not), so it is never retried: a 404 or a
 * request to sign in again is an answer, not a hiccup.
 */
export const adminOverviewQuery = () =>
  queryOptions({
    queryKey: ["admin", "system.admin_overview"] as const,
    queryFn: async ({ signal }) =>
      unwrap(await adminApi.GET("/admin/v1/system/overview", { signal })),
    retry: false,
    staleTime: 30_000,
  });
