import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, unwrapEmpty, webRoutes, type Schema } from "~/lib/api";

export type NewAccount = Schema<"NewAccount">;
export type HeldGrant = Schema<"HeldGrant">;

// Keyed by operationId (`operationKey`), so the change feed invalidates them.
const keys = {
  request: (id: string) => operationKey("identity.get_sign_in_request", { id }),
  newAccount: operationKey("identity.get_new_account"),
  heldGrants: operationKey("identity.list_held_grants"),
};

/**
 * `identity.get_sign_in_request`: the waiting authorization request the
 * sign-in screen was given, and the ways in on offer. 404 when it expired.
 */
export function useSignInRequest(id: string) {
  return useQuery({
    queryKey: keys.request(id),
    queryFn: async ({ signal }) =>
      unwrap(await webRoutes().GET("/auth/requests/{id}", { params: { path: { id } }, signal })),
    staleTime: Infinity,
  });
}

/** `identity.get_new_account`: the account this browser may create. 404 when none waits. */
export function useNewAccount() {
  return useQuery({
    queryKey: keys.newAccount,
    queryFn: async ({ signal }) => unwrap(await webRoutes().GET("/auth/new-account", { signal })),
    staleTime: Infinity,
  });
}

/** `identity.create_account`: creates it and signs in; resolves to where to go next. */
export function useCreateAccount() {
  return useMutation({
    mutationFn: async () =>
      unwrap(
        await webRoutes().POST("/auth/new-account", {
          // The issuer refuses screen actions without the CSRF header.
          params: { header: { "x-campbooks-csrf": "1" } },
        }),
      ).redirect,
  });
}

/** `identity.list_held_grants`: mailboxes from sign-up, waiting for a workspace. */
export function useHeldGrants() {
  return useQuery({
    queryKey: keys.heldGrants,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET("/api/v1/identity/held-grants", { signal })).items,
  });
}

const dropGrant = (grants: HeldGrant[] | undefined, id: string) =>
  grants?.filter((grant) => grant.id !== id);

/**
 * `identity.adopt_held_grant`: connects a held mailbox to a workspace. The
 * grant leaves the cached list and the workspace's connections refresh.
 */
export function useAdoptHeldGrant(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (grantId: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/identity/held-grants/{id}/adopt", {
          params: { path: { workspace_id: workspaceId, id: grantId } },
        }),
      ),
    onSuccess: async (_adopted, grantId) => {
      queryClient.setQueryData<HeldGrant[]>(keys.heldGrants, (grants) =>
        dropGrant(grants, grantId),
      );
      await queryClient.invalidateQueries({
        queryKey: operationKey("connectors.list_connections", { workspace_id: workspaceId }),
      });
    },
  });
}

/** `identity.discard_held_grant`: forgets a held mailbox. */
export function useDiscardHeldGrant() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (grantId: string) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/identity/held-grants/{id}", {
          params: { path: { id: grantId } },
        }),
      );
      return grantId;
    },
    onSuccess: (grantId) => {
      queryClient.setQueryData<HeldGrant[]>(keys.heldGrants, (grants) =>
        dropGrant(grants, grantId),
      );
    },
  });
}

export type WayIn = Schema<"WayIn">;
export type WaysIn = Schema<"WaysIn">;
export type StartLink = Schema<"StartLink">;

const waysInKey = operationKey("identity.list_ways_in");

/** `identity.list_ways_in`: the provider accounts you sign in with, and the providers on offer. */
export function useWaysIn() {
  return useQuery({
    queryKey: waysInKey,
    queryFn: async ({ signal }) => unwrap(await api.GET("/api/v1/identity/ways-in", { signal })),
  });
}

/** `identity.remove_way_in`: stop signing in with one (never the last; needs a recent sign-in). */
export function useRemoveWayIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      unwrapEmpty(await api.DELETE("/api/v1/identity/ways-in/{id}", { params: { path: { id } } }));
      return id;
    },
    onSuccess: (id) => {
      queryClient.setQueryData<WaysIn>(waysInKey, (ways) =>
        ways === undefined ? ways : { ...ways, items: ways.items.filter((way) => way.id !== id) },
      );
    },
  });
}

/**
 * `identity.start_link`: a single-use URL of the front door for adding a way
 * in, connecting a mailbox or reconnecting one; the browser goes there next.
 */
export function useStartLink() {
  return useMutation({
    mutationFn: async (body: StartLink) =>
      unwrap(await api.POST("/api/v1/identity/links", { body })).url,
  });
}

export type ConsentRequest = Schema<"ConsentRequest">;
export type Answer = Schema<"Answer">;

/**
 * `identity.get_consent_request`: what the consent screen shows for the
 * waiting request — the app, what it asks for, the workspaces it may use.
 * 404 when it expired, was answered, or is someone else's.
 */
export function useConsentRequest(id: string) {
  return useQuery({
    queryKey: operationKey("identity.get_consent_request", { id }),
    queryFn: async ({ signal }) =>
      unwrap(await webRoutes().GET("/auth/consent/{id}", { params: { path: { id } }, signal })),
    staleTime: Infinity,
    retry: false,
  });
}

/** `identity.answer_consent`: allow (in a workspace) or deny; resolves to where to go next. */
export function useAnswerConsent(id: string) {
  return useMutation({
    mutationFn: async (body: Answer) =>
      unwrap(
        await webRoutes().POST("/auth/consent/{id}", {
          // The issuer refuses screen actions without the CSRF header.
          params: { path: { id }, header: { "x-campbooks-csrf": "1" } },
          body,
        }),
      ).redirect,
  });
}

export type ConnectedApp = Schema<"ConnectedApp">;

const consentsKey = operationKey("identity.list_consents");

/** `identity.list_consents`: the apps you let in. */
export function useConnectedApps() {
  return useQuery({
    queryKey: consentsKey,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET("/api/v1/identity/consents", { signal })).items,
  });
}

/** `identity.revoke_consent`: takes an app's access away; its tokens stop working. */
export function useRevokeConsent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      unwrapEmpty(await api.DELETE("/api/v1/identity/consents/{id}", { params: { path: { id } } }));
      return id;
    },
    onSuccess: (id) => {
      queryClient.setQueryData<ConnectedApp[]>(consentsKey, (apps) =>
        apps?.filter((app) => app.id !== id),
      );
    },
  });
}

export type WorkspaceClient = Schema<"WorkspaceClient">;
export type RegisterClient = Schema<"RegisterClient">;
export type RegisteredClient = Schema<"RegisteredClient">;

const clientsKey = (workspaceId: string) =>
  operationKey("identity.list_clients", { workspace_id: workspaceId });

/** `identity.list_clients`: the workspace's own apps (owners and admins). */
export function useWorkspaceClients(workspaceId: string) {
  return useQuery({
    queryKey: clientsKey(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/identity/clients", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).items,
    retry: false,
  });
}

/** `identity.register_client`: a new app; a confidential one's secret is in the answer only. */
export function useRegisterClient(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: RegisterClient) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/identity/clients", {
          params: { path: { workspace_id: workspaceId } },
          body,
        }),
      ),
    onSuccess: (registered) => {
      queryClient.setQueryData<WorkspaceClient[]>(clientsKey(workspaceId), (clients) => [
        ...(clients ?? []),
        registered.client,
      ]);
    },
  });
}

/** `identity.rotate_client_secret`: a new secret, shown once; the old one stops at once. */
export function useRotateClientSecret(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (clientId: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/identity/clients/{client_id}/secret", {
          params: { path: { workspace_id: workspaceId, client_id: clientId } },
        }),
      ),
    onSuccess: (rotated) => {
      queryClient.setQueryData<WorkspaceClient[]>(clientsKey(workspaceId), (clients) =>
        clients?.map((client) =>
          client.client_id === rotated.client.client_id ? rotated.client : client,
        ),
      );
    },
  });
}

/** `identity.remove_client`: removes an app; its tokens stop working. */
export function useRemoveClient(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (clientId: string) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/identity/clients/{client_id}", {
          params: { path: { workspace_id: workspaceId, client_id: clientId } },
        }),
      );
      return clientId;
    },
    onSuccess: (clientId) => {
      queryClient.setQueryData<WorkspaceClient[]>(clientsKey(workspaceId), (clients) =>
        clients?.filter((client) => client.client_id !== clientId),
      );
    },
  });
}
