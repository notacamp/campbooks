import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, unwrap, unwrapEmpty, type Schema } from "~/lib/api";

export type Workspace = Schema<"WorkspaceView">;
export type Member = Schema<"MemberView">;
export type Invitation = Schema<"InvitationView">;
export type Role = Schema<"Role">;
export type Kind = Schema<"Kind">;

const keys = {
  list: ["workspaces", "list"] as const,
  members: (workspaceId: string) => ["workspaces", workspaceId, "members"] as const,
  invitations: (workspaceId: string) => ["workspaces", workspaceId, "invitations"] as const,
};

const path = (workspaceId: string) => ({ path: { workspace_id: workspaceId } });

/** `workspaces.list`: the caller's workspaces, oldest first, each with their role. */
const workspacesQuery = () =>
  queryOptions({
    queryKey: keys.list,
    queryFn: async ({ signal }) =>
      unwrap(await api.GET("/api/v1/workspaces", { signal })).workspaces,
  });

export function useWorkspaces() {
  return useQuery(workspacesQuery());
}

/** `workspaces.create`. The new workspace joins the cached list. */
export function useCreateWorkspace() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: Schema<"CreateWorkspace">) =>
      unwrap(await api.POST("/api/v1/workspaces", { body })),
    onSuccess: async (workspace) => {
      queryClient.setQueryData<Workspace[]>(keys.list, (list) =>
        list === undefined ? [workspace] : [...list, workspace],
      );
      await queryClient.invalidateQueries({ queryKey: keys.list });
    },
  });
}

/** `workspaces.list_members`. */
export function useMembers(workspaceId: string) {
  return useQuery({
    queryKey: keys.members(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/members", { params: path(workspaceId), signal }),
      ).members,
  });
}

/** `workspaces.list_invitations` (owners and admins only). */
export function useInvitations(workspaceId: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.invitations(workspaceId),
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/invitations", {
          params: path(workspaceId),
          signal,
        }),
      ).invitations,
  });
}

/** `workspaces.change_role`. Updates the cached member in place. */
export function useChangeRole(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ userId, role }: { userId: string; role: Role }) =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/members/{user_id}", {
          params: { path: { workspace_id: workspaceId, user_id: userId } },
          body: { role },
        }),
      ),
    onSuccess: (member) => {
      queryClient.setQueryData<Member[]>(keys.members(workspaceId), (members) =>
        members?.map((current) => (current.user_id === member.user_id ? member : current)),
      );
    },
  });
}

/** `workspaces.remove_member`. */
export function useRemoveMember(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (userId: string) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/members/{user_id}", {
          params: { path: { workspace_id: workspaceId, user_id: userId } },
        }),
      );
      return userId;
    },
    onSuccess: (userId) => {
      queryClient.setQueryData<Member[]>(keys.members(workspaceId), (members) =>
        members?.filter((member) => member.user_id !== userId),
      );
    },
  });
}

/** `workspaces.invite_member`. A new invitation to the same address replaces the old one. */
export function useInviteMember(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: Schema<"InviteMember">) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/invitations", { params: path(workspaceId), body }),
      ),
    onSuccess: (invitation) => {
      queryClient.setQueryData<Invitation[]>(keys.invitations(workspaceId), (invitations) => [
        invitation,
        ...(invitations ?? []).filter((current) => current.email !== invitation.email),
      ]);
    },
  });
}

/** `workspaces.accept_invitation`. The joined workspace joins the cached list. */
export function useAcceptInvitation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (token: string) =>
      unwrap(await api.POST("/api/v1/workspaces/invitations/accept", { body: { token } })),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: keys.list });
    },
  });
}
