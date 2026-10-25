import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  api,
  errorStatus,
  operationKey,
  unwrap,
  unwrapEmpty,
  type OperationQuery,
  type Schema,
} from "~/lib/api";

export type Connection = Schema<"ConnectionView">;

/**
 * `connectors.list_connections`: the workspace's connections the caller can
 * see (their own; all of them for owners and admins), oldest first.
 */
export function useConnections(workspaceId: string) {
  return useQuery({
    queryKey: operationKey("connectors.list_connections", { workspace_id: workspaceId }),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/connections", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).connections,
    // While a mailbox's address is being checked, its row follows it.
    refetchInterval: (query) =>
      query.state.data?.some((connection) => connection.address_check === "checking") === true
        ? 5000
        : false,
  });
}

/** What becomes of a disconnected mailbox's mail in Campbooks. */
export type MailChoice = NonNullable<OperationQuery<"connectors.disconnect">["mail"]>;

/**
 * `connectors.disconnect`: stops syncing for good and gives up the grant at
 * the provider; its mail in Campbooks is deleted or kept read-only (`mail`).
 * Only its grantor may. The connection leaves the cached list.
 */
export function useDisconnect(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ connectionId, mail }: { connectionId: string; mail: MailChoice }) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/connections/{connection_id}", {
          params: {
            path: { workspace_id: workspaceId, connection_id: connectionId },
            query: { mail },
          },
        }),
      );
      return connectionId;
    },
    onSuccess: (connectionId) => {
      queryClient.setQueryData<Connection[]>(
        operationKey("connectors.list_connections", { workspace_id: workspaceId }),
        (connections) => connections?.filter((connection) => connection.id !== connectionId),
      );
      // A way in may have gone with it, and its inboxes were deleted or kept.
      // Not awaited: the row that asked is gone from the list, and the
      // caller's own onSuccess must still run.
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: operationKey("identity.list_ways_in") }),
        queryClient.invalidateQueries({
          queryKey: operationKey("conversations.list_inboxes", { workspace_id: workspaceId }),
        }),
        queryClient.invalidateQueries({
          queryKey: operationKey("conversations.list", { workspace_id: workspaceId }),
        }),
      ]);
    },
    onError: (error) => {
      // Gone already (or never yours to see): the stale row goes. Not
      // awaited, so the caller hears of the failure before its row does.
      if (errorStatus(error) === 404) {
        void queryClient.invalidateQueries({
          queryKey: operationKey("connectors.list_connections", { workspace_id: workspaceId }),
        });
      }
    },
  });
}

export type DiscoveredImap = Schema<"DiscoveredImap">;
export type MailServer = Schema<"Server">;
export type MailServers = Schema<"Servers">;

/**
 * `connectors.discover_imap`: which way an address connects (its IMAP and
 * SMTP servers, a provider connected natively, or nothing found). Reads
 * nothing it keeps; needs a recent sign-in.
 */
export function useDiscoverImap(workspaceId: string) {
  return useMutation({
    mutationFn: async (address: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/connections/imap/discover", {
          params: { path: { workspace_id: workspaceId } },
          body: { address },
        }),
      ),
  });
}

/** What connecting a mailbox with its password sends. */
interface ImapConnect {
  address: string;
  username?: string | undefined;
  password: string;
  servers?: MailServers | undefined;
}

/**
 * `connectors.connect_imap`: connect a mailbox with its (app) password. A
 * plain call, never a `useMutation`: a mutation keeps its variables (the
 * password) in the query cache after it settles. The connection joins the
 * cached list.
 */
export async function connectImap(
  workspaceId: string,
  { address, username, password, servers }: ImapConnect,
): Promise<Connection> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/connections/imap", {
      params: { path: { workspace_id: workspaceId } },
      body: {
        address,
        password,
        ...(username === undefined || username.trim() === "" ? {} : { username: username.trim() }),
        ...(servers === undefined ? {} : { servers }),
      },
    }),
  );
}

/**
 * `connectors.get_connection`, asked again every few seconds while its
 * address is being checked.
 */
export function useConnection(workspaceId: string, connectionId: string) {
  return useQuery({
    queryKey: operationKey("connectors.get_connection", {
      workspace_id: workspaceId,
      connection_id: connectionId,
    }),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/connections/{connection_id}", {
          params: { path: { workspace_id: workspaceId, connection_id: connectionId } },
          signal,
        }),
      ),
    refetchInterval: (query) =>
      query.state.data === undefined || query.state.data.address_check === "checking"
        ? 3000
        : false,
  });
}

/**
 * `connectors.check_address`: check an IMAP mailbox's address again (a new
 * code through its own SMTP server). The answer replaces the cached row.
 */
export function useCheckAddress(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (connectionId: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/connections/{connection_id}/address-check", {
          params: { path: { workspace_id: workspaceId, connection_id: connectionId } },
        }),
      ),
    onSuccess: (connection) => {
      remember(queryClient, workspaceId, connection);
    },
  });
}

/** Put `connection` in the cached list (and its own cache entry). */
export function remember(
  queryClient: ReturnType<typeof useQueryClient>,
  workspaceId: string,
  connection: Connection,
) {
  queryClient.setQueryData<Connection[]>(
    operationKey("connectors.list_connections", { workspace_id: workspaceId }),
    (connections) =>
      connections === undefined
        ? connections
        : connections.some((known) => known.id === connection.id)
          ? connections.map((known) => (known.id === connection.id ? connection : known))
          : [...connections, connection],
  );
  queryClient.setQueryData(
    operationKey("connectors.get_connection", {
      workspace_id: workspaceId,
      connection_id: connection.id,
    }),
    connection,
  );
}
