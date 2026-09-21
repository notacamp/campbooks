/**
 * modules/settings/api/use-notifications-query — GET /api/app/notifications
 *
 * Paginated list of in-app notifications with filter support.
 * filter: "all" | "needs_action" | "unread" | "archived"
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiCollection } from "~/lib/api";
import type { NotificationItem } from "~/modules/settings/types";
import type { CollectionEnvelope } from "~/lib/api";

export type NotificationsFilter = "all" | "needs_action" | "unread" | "archived";

export const notificationsKeys = {
  all: (filter: NotificationsFilter) =>
    ["settings", "notifications", filter] as const,
};

export const useNotificationsQuery = (
  filter: NotificationsFilter = "all",
): UseQueryResult<CollectionEnvelope<NotificationItem>, Error> =>
  useQuery<CollectionEnvelope<NotificationItem>, Error>({
    queryKey: notificationsKeys.all(filter),
    queryFn: () =>
      apiCollection<NotificationItem>(`/notifications?filter=${filter}`),
    staleTime: 30_000,
  });
