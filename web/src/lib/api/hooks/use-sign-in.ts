/**
 * useSignInMutation — mutation for POST /api/app/session.
 *
 * On success: stores the bearer token via setToken and returns the response.
 * Throws ApiError on failure:
 *   401 invalid_credentials — wrong email/password.
 *   401 mfa_required        — MFA challenge required (not supported in this client).
 */
import { useMutation, type UseMutationResult } from "@tanstack/react-query";
import { apiClient } from "../client";
import { setToken } from "../token";

export interface SignInParams {
  email_address: string;
  password: string;
}

export interface SignInResponse {
  token: string;
  expires_at: string;
}

export const useSignInMutation = (): UseMutationResult<
  SignInResponse,
  Error,
  SignInParams
> =>
  useMutation<SignInResponse, Error, SignInParams>({
    mutationFn: async (params) => {
      const data = await apiClient<SignInResponse>("/session", {
        method: "POST",
        body: params,
      });
      setToken(data.token);
      return data;
    },
  });
