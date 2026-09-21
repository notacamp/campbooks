/**
 * modules/settings — public barrel for the Settings module.
 *
 * Exports the module config (for modules.ts registration) and the public
 * API surface (types, hooks) that routes consume.
 *
 * Routes are wired in router.tsx via buildSettingsRoutes (routes/settings/index.ts).
 */
import type { ModuleConfig } from "~/modules";

export const moduleConfig: ModuleConfig = {
  extensions: [],
};

// Public types
export type { AccountData, WorkspaceData, PrivacyData } from "./types";

// Data hooks
export {
  useAccountQuery,
  accountKeys,
  useAccountLanguageMutation,
  useAccountComposeMutation,
  useAccountWritingStyleMutation,
  useAccountPasswordMutation,
  useWorkspaceQuery,
  workspaceKeys,
  useWorkspaceMutation,
  usePrivacyQuery,
  privacyKeys,
  usePrivacyMutation,
} from "./api";
