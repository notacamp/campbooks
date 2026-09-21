/**
 * modules/settings/api/use-workspace-mutation — PATCH /api/app/settings/workspace
 *
 * Admin-only mutation. Updates workspace name, AI persona context,
 * and company NIF.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { WorkspaceData } from "~/modules/settings/types";
import { workspaceKeys } from "./use-workspace-query";

export interface WorkspaceUpdateParams {
  name?: string;
  workspace_context?: string;
  company_nif?: string;
}

export const useWorkspaceMutation = (): UseMutationResult<
  WorkspaceData,
  Error,
  WorkspaceUpdateParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: WorkspaceUpdateParams) =>
      apiClient<WorkspaceData>("/settings/workspace", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(workspaceKeys.all, data);
    },
  });
};
