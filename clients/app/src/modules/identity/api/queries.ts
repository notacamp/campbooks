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
