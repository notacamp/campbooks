/**
 * modules/settings/api/use-integrations-mutation — integrations write hooks.
 *
 * Covers Notion connect/disconnect and Connections CRUD.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { NotionIntegrationsData, ConnectionData } from "~/modules/settings/types";
import { integrationsKeys } from "./use-integrations-query";

// ── Notion: connect via manual token ─────────────────────────────────────────

export interface NotionConnectParams {
  access_token: string;
}

export const useNotionConnectMutation = (): UseMutationResult<
  NotionIntegrationsData,
  Error,
  NotionConnectParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: NotionConnectParams) =>
      apiClient<NotionIntegrationsData>("/settings/integrations/notion", {
        method: "PATCH",
        body: { notion_integration: params },
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(integrationsKeys.notion, data);
      void queryClient.invalidateQueries({
        queryKey: integrationsKeys.overview,
      });
    },
  });
};

// ── Notion: disconnect workspace ──────────────────────────────────────────────

export const useNotionDisconnectMutation = (): UseMutationResult<
  void,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/settings/integrations/notion/workspaces/${id}`, {
        method: "DELETE",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: integrationsKeys.notion });
      void queryClient.invalidateQueries({
        queryKey: integrationsKeys.overview,
      });
    },
  });
};

// ── Connections: create ───────────────────────────────────────────────────────

export interface ConnectionCreateParams {
  name: string;
  base_url: string;
  auth_type: string;
  auth_header_name?: string;
  auth_username?: string;
  auth_secret?: string;
}

export const useCreateConnectionMutation = (): UseMutationResult<
  ConnectionData,
  Error,
  ConnectionCreateParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: ConnectionCreateParams) =>
      apiClient<ConnectionData>("/settings/integrations/connections", {
        method: "POST",
        body: { connection: params },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: integrationsKeys.connections,
      });
    },
  });
};

// ── Connections: update ───────────────────────────────────────────────────────

export interface ConnectionUpdateParams extends Partial<ConnectionCreateParams> {
  id: number;
}

export const useUpdateConnectionMutation = (): UseMutationResult<
  ConnectionData,
  Error,
  ConnectionUpdateParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...params }: ConnectionUpdateParams) =>
      apiClient<ConnectionData>(`/settings/integrations/connections/${id}`, {
        method: "PATCH",
        body: { connection: params },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: integrationsKeys.connections,
      });
    },
  });
};

// ── Connections: destroy ──────────────────────────────────────────────────────

export const useDeleteConnectionMutation = (): UseMutationResult<
  void,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/settings/integrations/connections/${id}`, {
        method: "DELETE",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: integrationsKeys.connections,
      });
    },
  });
};
