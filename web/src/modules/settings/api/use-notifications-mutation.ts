/**
 * modules/settings/api/use-notifications-mutation — per-item and bulk mutations.
 *
 * All mutations invalidate the notifications query keys so the list stays fresh.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { NotificationItem } from "~/modules/settings/types";
import { notificationsKeys } from "./use-notifications-query";

const FILTER_KEYS = ["all", "needs_action", "unread", "archived"] as const;

const invalidateAll = (queryClient: ReturnType<typeof useQueryClient>): void => {
  FILTER_KEYS.forEach((f) => {
    void queryClient.invalidateQueries({ queryKey: notificationsKeys.all(f) });
  });
};

// ── Per-item mark_read ────────────────────────────────────────────────────────

export const useMarkReadMutation = (): UseMutationResult<
  NotificationItem,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<NotificationItem>(`/notifications/${id}/mark_read`, {
        method: "POST",
      }),
    onSuccess: () => invalidateAll(queryClient),
  });
};

// ── Per-item archive ──────────────────────────────────────────────────────────

export const useArchiveNotificationMutation = (): UseMutationResult<
  NotificationItem,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<NotificationItem>(`/notifications/${id}/archive`, {
        method: "POST",
      }),
    onSuccess: () => invalidateAll(queryClient),
  });
};

// ── Per-item unarchive ────────────────────────────────────────────────────────

export const useUnarchiveNotificationMutation = (): UseMutationResult<
  NotificationItem,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<NotificationItem>(`/notifications/${id}/unarchive`, {
        method: "POST",
      }),
    onSuccess: () => invalidateAll(queryClient),
  });
};

// ── Bulk mark_all_read ────────────────────────────────────────────────────────

export const useMarkAllReadMutation = (): UseMutationResult<void, Error, void> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () =>
      apiClient<void>("/notifications/mark_all_read", { method: "POST" }),
    onSuccess: () => invalidateAll(queryClient),
  });
};

// ── Bulk archive_all ──────────────────────────────────────────────────────────

export const useArchiveAllMutation = (): UseMutationResult<void, Error, void> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () =>
      apiClient<void>("/notifications/archive_all", { method: "POST" }),
    onSuccess: () => invalidateAll(queryClient),
  });
};
