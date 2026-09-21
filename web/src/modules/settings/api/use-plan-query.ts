/**
 * modules/settings/api/use-plan-query — GET /api/app/settings/plan
 *
 * Read-only plan + entitlements snapshot. Returns current plan name, per-feature
 * flags, and per-resource limits (members, email_accounts, api_clients).
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { PlanData } from "~/modules/settings/types";

export const planKeys = {
  all: ["settings", "plan"] as const,
};

export const usePlanQuery = (): UseQueryResult<PlanData, Error> =>
  useQuery<PlanData, Error>({
    queryKey: planKeys.all,
    queryFn: () => apiClient<PlanData>("/settings/plan"),
    staleTime: 5 * 60_000,
  });
