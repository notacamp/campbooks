/**
 * modules/settings/api/use-notification-preferences-mutation
 *
 * Three mutations:
 *   - toggle    PATCH /api/app/notification_preferences/toggle
 *   - bulk      PATCH /api/app/notification_preferences/bulk_toggle
 *   - digest    PATCH /api/app/settings/notifications/digest_preference
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { NotificationPrefItem } from "~/modules/settings/types";
import { notificationPreferencesKeys } from "./use-notification-preferences-query";

// ── Toggle ────────────────────────────────────────────────────────────────────

export interface NotificationPrefToggleParams {
  kind: "tag" | "document_type";
  tag_id?: number;
  document_type_id?: number;
  notify_in_app?: boolean;
  notify_email?: boolean;
}

export const useNotificationPrefToggleMutation = (): UseMutationResult<
  NotificationPrefItem,
  Error,
  NotificationPrefToggleParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: NotificationPrefToggleParams) =>
      apiClient<NotificationPrefItem>("/notification_preferences/toggle", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: notificationPreferencesKeys.all,
      });
    },
  });
};

// ── Bulk toggle ───────────────────────────────────────────────────────────────

export interface NotificationBulkToggleParams {
  kind: "tag" | "document_type";
  channel: "in_app" | "email";
  value: boolean;
}

export const useNotificationBulkToggleMutation = (): UseMutationResult<
  { updated: boolean },
  Error,
  NotificationBulkToggleParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: NotificationBulkToggleParams) =>
      apiClient<{ updated: boolean }>(
        "/notification_preferences/bulk_toggle",
        {
          method: "PATCH",
          body: params,
        },
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: notificationPreferencesKeys.all,
      });
    },
  });
};

// ── Digest preference ─────────────────────────────────────────────────────────

export interface DigestPreferenceParams {
  email_on_waiting_on_replies_digest: boolean;
}

export const useDigestPreferenceMutation = (): UseMutationResult<
  { digest_preference: boolean },
  Error,
  DigestPreferenceParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: DigestPreferenceParams) =>
      apiClient<{ digest_preference: boolean }>(
        "/settings/notifications/digest_preference",
        {
          method: "PATCH",
          body: params,
        },
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: notificationPreferencesKeys.all,
      });
    },
  });
};
