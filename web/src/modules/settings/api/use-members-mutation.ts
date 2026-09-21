/**
 * modules/settings/api/use-members-mutation — role updates + invitation CRUD.
 *
 * All mutations invalidate the members query so the list refreshes.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { MemberData, InvitationData } from "~/modules/settings/types";
import { membersKeys } from "./use-members-query";

// ── Change role ───────────────────────────────────────────────────────────────

export interface MemberRoleParams {
  id: number;
  role: string;
}

export const useMemberRoleMutation = (): UseMutationResult<
  MemberData,
  Error,
  MemberRoleParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, role }: MemberRoleParams) =>
      apiClient<MemberData>(`/settings/members/${id}`, {
        method: "PATCH",
        body: { role },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: membersKeys.all });
    },
  });
};

// ── Create invitation ─────────────────────────────────────────────────────────

export interface CreateInvitationParams {
  email: string;
}

export const useCreateInvitationMutation = (): UseMutationResult<
  InvitationData,
  Error,
  CreateInvitationParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateInvitationParams) =>
      apiClient<InvitationData>("/settings/invitations", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: membersKeys.all });
    },
  });
};

// ── Cancel (destroy) invitation ───────────────────────────────────────────────

export const useCancelInvitationMutation = (): UseMutationResult<
  void,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/settings/invitations/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: membersKeys.all });
    },
  });
};

// ── Resend invitation ─────────────────────────────────────────────────────────

export const useResendInvitationMutation = (): UseMutationResult<
  InvitationData,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<InvitationData>(`/settings/invitations/${id}/resend`, {
        method: "POST",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: membersKeys.all });
    },
  });
};

// ── Approve invitation ────────────────────────────────────────────────────────

export const useApproveInvitationMutation = (): UseMutationResult<
  InvitationData,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<InvitationData>(`/settings/invitations/${id}/approve`, {
        method: "POST",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: membersKeys.all });
    },
  });
};
