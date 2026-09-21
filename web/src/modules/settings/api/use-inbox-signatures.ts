/**
 * modules/settings/api/use-inbox-signatures — hooks for /api/app/inbox_settings/signatures
 */
import {
  useQuery,
  useMutation,
  useQueryClient,
  type UseQueryResult,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type {
  Signature,
  CreateSignatureParams,
  UpdateSignatureParams,
} from "~/routes/settings/inbox/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxSignaturesKeys = {
  all: ["settings", "inbox", "signatures"] as const,
};

// ── List ──────────────────────────────────────────────────────────────────────

export const useSignaturesQuery = (): UseQueryResult<Signature[], Error> =>
  useQuery<Signature[], Error>({
    queryKey: inboxSignaturesKeys.all,
    queryFn: () => apiClient<Signature[]>("/inbox_settings/signatures"),
    staleTime: 2 * 60_000,
  });

// ── Create ────────────────────────────────────────────────────────────────────

export const useCreateSignatureMutation = (): UseMutationResult<
  Signature,
  Error,
  CreateSignatureParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateSignatureParams) =>
      apiClient<Signature>("/inbox_settings/signatures", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxSignaturesKeys.all });
    },
  });
};

// ── Update ────────────────────────────────────────────────────────────────────

export const useUpdateSignatureMutation = (): UseMutationResult<
  Signature,
  Error,
  UpdateSignatureParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, signature }: UpdateSignatureParams) =>
      apiClient<Signature>(`/inbox_settings/signatures/${id}`, {
        method: "PATCH",
        body: { signature },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxSignaturesKeys.all });
    },
  });
};

// ── Delete ────────────────────────────────────────────────────────────────────

export const useDeleteSignatureMutation = (): UseMutationResult<
  void,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/inbox_settings/signatures/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxSignaturesKeys.all });
    },
  });
};

// ── Set default ───────────────────────────────────────────────────────────────

export const useSetDefaultSignatureMutation = (): UseMutationResult<
  Signature,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<Signature>(`/inbox_settings/signatures/${id}/set_default`, {
        method: "POST",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxSignaturesKeys.all });
    },
  });
};
