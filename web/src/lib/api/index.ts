/**
 * lib/api — public barrel.
 *
 * Modules import their data through their own api/ hooks (useQuery wrappers),
 * which in turn call apiClient from this lib. Direct apiClient imports in
 * modules/ are banned by ESLint no-restricted-imports.
 *
 * Exports here are what module api/ layers and other lib/* may use.
 */
export { apiClient, apiCollection, ApiError } from "./client";
export type {
  ApiMeta,
  ApiSuccessEnvelope,
  ApiErrorEnvelope,
  CollectionMeta,
  CollectionEnvelope,
} from "./client";
export { createQueryClient } from "./query-client";
export { useMe } from "./hooks/use-me";
export type { Me } from "./hooks/use-me";
export { getToken, setToken, clearToken } from "./token";
export { useSignInMutation } from "./hooks/use-sign-in";
export type { SignInParams, SignInResponse } from "./hooks/use-sign-in";
