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

export { usePlanQuery, planKeys } from "./use-plan-query";

export {
  useNotificationsQuery,
  notificationsKeys,
} from "./use-notifications-query";
export type { NotificationsFilter } from "./use-notifications-query";

export {
  useMarkReadMutation,
  useArchiveNotificationMutation,
  useUnarchiveNotificationMutation,
  useMarkAllReadMutation,
  useArchiveAllMutation,
} from "./use-notifications-mutation";

export {
  useNotificationPreferencesQuery,
  notificationPreferencesKeys,
} from "./use-notification-preferences-query";

export {
  useNotificationPrefToggleMutation,
  useNotificationBulkToggleMutation,
  useDigestPreferenceMutation,
} from "./use-notification-preferences-mutation";
export type {
  NotificationPrefToggleParams,
  NotificationBulkToggleParams,
  DigestPreferenceParams,
} from "./use-notification-preferences-mutation";

export { useMembersQuery, membersKeys } from "./use-members-query";

export {
  useMemberRoleMutation,
  useCreateInvitationMutation,
  useCancelInvitationMutation,
  useResendInvitationMutation,
  useApproveInvitationMutation,
} from "./use-members-mutation";
export type {
  MemberRoleParams,
  CreateInvitationParams,
} from "./use-members-mutation";

export {
  useIntegrationsOverviewQuery,
  useNotionIntegrationsQuery,
  useCalendarsIntegrationsQuery,
  useConnectionsQuery,
  integrationsKeys,
} from "./use-integrations-query";

export {
  useNotionConnectMutation,
  useNotionDisconnectMutation,
  useCreateConnectionMutation,
  useUpdateConnectionMutation,
  useDeleteConnectionMutation,
} from "./use-integrations-mutation";
export type {
  NotionConnectParams,
  ConnectionCreateParams,
  ConnectionUpdateParams,
} from "./use-integrations-mutation";
