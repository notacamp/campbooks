/**
 * modules/settings/api/use-workspace-query — GET /api/app/settings/workspace
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { WorkspaceData } from "~/modules/settings/types";

export const workspaceKeys = {
  all: ["settings", "workspace"] as const,
};

export const useWorkspaceQuery = (): UseQueryResult<WorkspaceData, Error> =>
  useQuery<WorkspaceData, Error>({
    queryKey: workspaceKeys.all,
    queryFn: () => apiClient<WorkspaceData>("/settings/workspace"),
    staleTime: 5 * 60_000,
  });
