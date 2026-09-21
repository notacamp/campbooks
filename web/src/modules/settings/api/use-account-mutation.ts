/**
 * modules/settings/api/use-account-mutation — PATCH mutations for the account.
 *
 * Each endpoint is a separate mutation so callers can track per-field pending
 * state independently. On success each mutation writes the updated AccountData
 * into the account query cache directly (no refetch needed).
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { AccountData } from "~/modules/settings/types";
import { accountKeys } from "./use-account-query";

// ── Language ──────────────────────────────────────────────────────────────────

export interface AccountLanguageParams {
  locale: string;
}

export const useAccountLanguageMutation = (): UseMutationResult<
  AccountData,
  Error,
  AccountLanguageParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: AccountLanguageParams) =>
      apiClient<AccountData>("/account/language", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(accountKeys.all, data);
    },
  });
};

// ── Compose preference ────────────────────────────────────────────────────────

export interface AccountComposePreferenceParams {
  compose_default: string;
}

export const useAccountComposeMutation = (): UseMutationResult<
  AccountData,
  Error,
  AccountComposePreferenceParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: AccountComposePreferenceParams) =>
      apiClient<AccountData>("/account/compose_preference", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(accountKeys.all, data);
    },
  });
};

// ── Writing style ─────────────────────────────────────────────────────────────

export interface AccountWritingStyleParams {
  writing_style: string;
}

export const useAccountWritingStyleMutation = (): UseMutationResult<
  AccountData,
  Error,
  AccountWritingStyleParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: AccountWritingStyleParams) =>
      apiClient<AccountData>("/account/writing_style", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(accountKeys.all, data);
    },
  });
};

// ── Password change ───────────────────────────────────────────────────────────

export interface AccountPasswordParams {
  current_password: string;
  password: string;
  password_confirmation: string;
}

export const useAccountPasswordMutation = (): UseMutationResult<
  AccountData,
  Error,
  AccountPasswordParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: AccountPasswordParams) =>
      apiClient<AccountData>("/account", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(accountKeys.all, data);
    },
  });
};
