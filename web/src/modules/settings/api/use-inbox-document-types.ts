/**
 * modules/settings/api/use-inbox-document-types — hooks for /api/app/inbox_settings/document_types
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
  DocumentType,
  CreateDocumentTypeParams,
  UpdateDocumentTypeParams,
} from "~/routes/settings/inbox/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxDocumentTypesKeys = {
  all: ["settings", "inbox", "document_types"] as const,
};

// ── List ──────────────────────────────────────────────────────────────────────

export const useDocumentTypesQuery = (): UseQueryResult<DocumentType[], Error> =>
  useQuery<DocumentType[], Error>({
    queryKey: inboxDocumentTypesKeys.all,
    queryFn: () => apiClient<DocumentType[]>("/inbox_settings/document_types"),
    staleTime: 2 * 60_000,
  });

// ── Create ────────────────────────────────────────────────────────────────────

export const useCreateDocumentTypeMutation = (): UseMutationResult<
  DocumentType,
  Error,
  CreateDocumentTypeParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateDocumentTypeParams) =>
      apiClient<DocumentType>("/inbox_settings/document_types", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxDocumentTypesKeys.all });
    },
  });
};

// ── Update ────────────────────────────────────────────────────────────────────

export const useUpdateDocumentTypeMutation = (): UseMutationResult<
  DocumentType,
  Error,
  UpdateDocumentTypeParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, document_type }: UpdateDocumentTypeParams) =>
      apiClient<DocumentType>(`/inbox_settings/document_types/${id}`, {
        method: "PATCH",
        body: { document_type },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxDocumentTypesKeys.all });
    },
  });
};

// ── Delete ────────────────────────────────────────────────────────────────────

export const useDeleteDocumentTypeMutation = (): UseMutationResult<
  void,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/inbox_settings/document_types/${id}`, {
        method: "DELETE",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxDocumentTypesKeys.all });
    },
  });
};
