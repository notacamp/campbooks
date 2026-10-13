import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, unwrapEmpty, webRoutes, type Schema } from "~/lib/api";

import { createPasskey, type AuthenticationJSON, type RequestOptions } from "../webauthn";

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

// --- Passkeys ----------------------------------------------------------------

export type PasskeyView = Schema<"PasskeyView">;
type Passkeys = Schema<"Passkeys">;

const passkeysKey = operationKey("identity.list_passkeys");

/** The sign-in screens' actions carry the CSRF header; the issuer refuses them without it. */
const SCREEN_ACTION = { header: { "x-campbooks-csrf": "1" } } as const;

/**
 * Sign in with a passkey from the sign-in screen: `identity.start_passkey_sign_in`
 * for the options, `get` to have the browser sign them, then
 * `identity.finish_passkey_sign_in`. Resolves to where to go next.
 */
export async function signInWithPasskey(
  start: Schema<"SignInStart">,
  get: (options: RequestOptions) => Promise<AuthenticationJSON>,
): Promise<string> {
  const options = unwrap(
    await webRoutes().POST("/auth/passkey/sign-in/options", {
      params: SCREEN_ACTION,
      body: start,
    }),
  );
  const credential = await get(options);
  return unwrap(
    await webRoutes().POST("/auth/passkey/sign-in", { params: SCREEN_ACTION, body: credential }),
  ).redirect;
}

/**
 * Create an account with a passkey alone: `identity.start_passkey_sign_up`,
 * a new passkey from the browser, `identity.finish_passkey_sign_up`. Resolves
 * to where to go next.
 */
export function usePasskeySignUp() {
  return useMutation({
    mutationFn: async (start: Schema<"SignUpStart">) => {
      const options = unwrap(
        await webRoutes().POST("/auth/passkey/sign-up/options", {
          params: SCREEN_ACTION,
          body: start,
        }),
      );
      const credential = await createPasskey(options);
      return unwrap(
        await webRoutes().POST("/auth/passkey/sign-up", {
          params: SCREEN_ACTION,
          body: credential,
        }),
      ).redirect;
    },
  });
}

/** `identity.list_passkeys`: the passkeys you sign in with. */
export function usePasskeys() {
  return useQuery({
    queryKey: passkeysKey,
    queryFn: async ({ signal }) => unwrap(await api.GET("/api/v1/identity/passkeys", { signal })),
  });
}

/**
 * Add a passkey: `identity.start_passkey_registration` (needs a recent
 * sign-in), a new passkey from the browser, then `identity.register_passkey`.
 */
export function useAddPasskey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (name: string | undefined) => {
      const options = unwrap(await api.POST("/api/v1/identity/passkeys/registrations"));
      const credential = await createPasskey(options);
      return unwrap(
        await api.POST("/api/v1/identity/passkeys", {
          body: { credential, ...(name === undefined ? {} : { name }) },
        }),
      );
    },
    onSuccess: (added) => {
      queryClient.setQueryData<Passkeys>(passkeysKey, (passkeys) =>
        passkeys === undefined ? passkeys : { items: [...passkeys.items, added] },
      );
    },
  });
}

/** `identity.rename_passkey`. */
export function useRenamePasskey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, name }: { id: string; name: string }) =>
      unwrap(
        await api.PATCH("/api/v1/identity/passkeys/{id}", {
          params: { path: { id } },
          body: { name },
        }),
      ),
    onSuccess: (renamed) => {
      queryClient.setQueryData<Passkeys>(passkeysKey, (passkeys) =>
        passkeys === undefined
          ? passkeys
          : { items: passkeys.items.map((p) => (p.id === renamed.id ? renamed : p)) },
      );
    },
  });
}

/** `identity.remove_passkey`: never the last way in; needs a recent sign-in. */
export function useRemovePasskey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      unwrapEmpty(await api.DELETE("/api/v1/identity/passkeys/{id}", { params: { path: { id } } }));
      return id;
    },
    onSuccess: (id) => {
      queryClient.setQueryData<Passkeys>(passkeysKey, (passkeys) =>
        passkeys === undefined ? passkeys : { items: passkeys.items.filter((p) => p.id !== id) },
      );
    },
  });
}
