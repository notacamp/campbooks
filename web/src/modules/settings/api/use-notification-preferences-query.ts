/**
 * modules/settings/api/use-notification-preferences-query
 * GET /api/app/notification_preferences
 *
 * Returns digest toggle + per-tag and per-document-type in_app/email preferences.
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { NotificationPreferencesData } from "~/modules/settings/types";

export const notificationPreferencesKeys = {
  all: ["settings", "notification_preferences"] as const,
};

export const useNotificationPreferencesQuery =
  (): UseQueryResult<NotificationPreferencesData, Error> =>
    useQuery<NotificationPreferencesData, Error>({
      queryKey: notificationPreferencesKeys.all,
      queryFn: () =>
        apiClient<NotificationPreferencesData>("/notification_preferences"),
      staleTime: 2 * 60_000,
    });
