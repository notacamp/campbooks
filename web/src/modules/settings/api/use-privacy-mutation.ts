/**
 * modules/settings/api/use-privacy-mutation — PATCH /api/app/settings/data_privacy
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { PrivacyData } from "~/modules/settings/types";
import { privacyKeys } from "./use-privacy-query";

export interface PrivacyUpdateParams {
  ai_processing_enabled?: boolean;
  scout_thread_posts?: boolean;
  required_data_region?: string;
  email_retention_months?: number;
}

export const usePrivacyMutation = (): UseMutationResult<
  PrivacyData,
  Error,
  PrivacyUpdateParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: PrivacyUpdateParams) =>
      apiClient<PrivacyData>("/settings/data_privacy", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(privacyKeys.all, data);
    },
  });
};
