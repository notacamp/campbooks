/**
 * modules/settings/api — barrel for all settings data hooks.
 */

export { useAccountQuery, accountKeys } from "./use-account-query";
export {
  useAccountLanguageMutation,
  useAccountComposeMutation,
  useAccountWritingStyleMutation,
  useAccountPasswordMutation,
} from "./use-account-mutation";
export type {
  AccountLanguageParams,
  AccountComposePreferenceParams,
  AccountWritingStyleParams,
  AccountPasswordParams,
} from "./use-account-mutation";

export { useWorkspaceQuery, workspaceKeys } from "./use-workspace-query";
export { useWorkspaceMutation } from "./use-workspace-mutation";
export type { WorkspaceUpdateParams } from "./use-workspace-mutation";

export { usePrivacyQuery, privacyKeys } from "./use-privacy-query";
export { usePrivacyMutation } from "./use-privacy-mutation";
export type { PrivacyUpdateParams } from "./use-privacy-mutation";
