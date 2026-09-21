/**
 * NotificationsPage — Settings → Notifications at /settings/notifications.
 * Stub — not yet implemented. Backed by GET/PATCH /api/app/notification_preferences
 * and PATCH /api/app/settings/notifications/digest_preference.
 */
import { type FC } from "react";
import { StubPage } from "./StubPage";

export const NotificationsPage: FC = () => (
  <StubPage
    title="Notifications"
    description="Choose how and when Campbooks notifies you — digests, email alerts, and per-event preferences."
  />
);
