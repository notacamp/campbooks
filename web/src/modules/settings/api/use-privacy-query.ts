/**
 * modules/settings/api/use-privacy-query — GET /api/app/settings/data_privacy
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { PrivacyData } from "~/modules/settings/types";

export const privacyKeys = {
  all: ["settings", "privacy"] as const,
};

export const usePrivacyQuery = (): UseQueryResult<PrivacyData, Error> =>
  useQuery<PrivacyData, Error>({
    queryKey: privacyKeys.all,
    queryFn: () => apiClient<PrivacyData>("/settings/data_privacy"),
    staleTime: 5 * 60_000,
  });
