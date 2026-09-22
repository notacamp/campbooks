/**
 * NotificationsPage — Settings → Notifications at /settings/notifications.
 *
 * Two sections:
 *   1. Notification center — list of in-app notifications with filter tabs,
 *      per-item mark_read/archive/unarchive actions, and bulk mark_all_read /
 *      archive_all.
 *   2. Preferences — per-tag + per-document-type in_app/email toggles, bulk
 *      enable/disable per channel, and the email digest toggle.
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, Check, Bell, BellOff, Archive, Mail, Inbox } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import {
  useNotificationsQuery,
  useMarkReadMutation,
  useArchiveNotificationMutation,
  useUnarchiveNotificationMutation,
  useMarkAllReadMutation,
  useArchiveAllMutation,
  useNotificationPreferencesQuery,
  useNotificationPrefToggleMutation,
  useNotificationBulkToggleMutation,
  useDigestPreferenceMutation,
} from "~/modules/settings/api";
import type {
  NotificationsFilter,
  NotificationPrefToggleParams,
} from "~/modules/settings/api";
import type { NotificationItem, NotificationPrefItem } from "~/modules/settings/types";
import { cn } from "~/lib/utils";

// ── Shared helpers ────────────────────────────────────────────────────────────

const FieldError: FC<{ error: Error | null }> = ({ error }) => {
  if (!error) return null;
  const msg = error instanceof ApiError ? error.message : "Something went wrong";
  return (
    <p className="text-[12px] text-danger" role="alert">
      {msg}
    </p>
  );
};

// ── Toggle ────────────────────────────────────────────────────────────────────

interface ToggleProps {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}

const Toggle: FC<ToggleProps> = ({ checked, onChange, label, disabled }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    disabled={disabled}
    onClick={() => onChange(!checked)}
    className={cn(
      "relative inline-flex items-center h-5 w-9 rounded-full shrink-0",
      "focus:outline-none focus:ring-2 focus:ring-[--ring] focus:ring-offset-2",
      "transition-colors duration-[--cb-dur]",
      checked ? "bg-accent-cb" : "bg-surface-3 border border-line",
      "disabled:opacity-40 disabled:cursor-not-allowed",
    )}
  >
    <span
      className={cn(
        "inline-block w-3.5 h-3.5 rounded-full bg-white shadow-sm",
        "transform transition-transform duration-[--cb-dur]",
        checked ? "translate-x-[18px]" : "translate-x-0.5",
      )}
    />
  </button>
);

// ── SECTION 1: Notification center ───────────────────────────────────────────

const FILTER_TABS: { value: NotificationsFilter; label: string }[] = [
  { value: "all",          label: "All" },
  { value: "needs_action", label: "Action needed" },
  { value: "unread",       label: "Unread" },
  { value: "archived",     label: "Archived" },
];

const CATEGORY_ICONS: Record<string, React.ReactNode> = {
  default: <Bell size={14} className="text-t3" />,
};

interface NotifRowProps {
  notification: NotificationItem;
  onMarkRead: (id: number) => void;
  onArchive: (id: number) => void;
  onUnarchive: (id: number) => void;
  isPending: boolean;
}

const NotifRow: FC<NotifRowProps> = ({
  notification: n,
  onMarkRead,
  onArchive,
  onUnarchive,
  isPending,
}) => (
  <div
    className={cn(
      "flex items-start gap-3 py-3 border-b border-line last:border-b-0",
      !n.read && "bg-surface-2",
    )}
  >
    <div className="mt-0.5 shrink-0">
      {CATEGORY_ICONS[n.category] ?? CATEGORY_ICONS.default}
    </div>
    <div className="flex-1 min-w-0">
      <p
        className={cn(
          "text-[13px] leading-snug",
          n.read ? "text-t2" : "text-t1 font-[500]",
        )}
      >
        {n.title}
      </p>
      {n.body && (
        <p className="text-[12px] text-t3 mt-0.5 line-clamp-2">{n.body}</p>
      )}
      <p className="text-[11px] text-t4 mt-1">
        {new Date(n.created_at).toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
          year: "numeric",
        })}
      </p>
    </div>
    <div className="flex items-center gap-1.5 shrink-0">
      {!n.read && (
        <button
          type="button"
          aria-label="Mark as read"
          onClick={() => onMarkRead(n.id)}
          disabled={isPending}
          className={cn(
            "p-1.5 rounded-cb-1 text-t4 hover:text-t1 hover:bg-surface-3",
            "transition-colors duration-[--cb-dur] disabled:opacity-40",
          )}
        >
          <Check size={13} strokeWidth={2.5} />
        </button>
      )}
      {!n.archived ? (
        <button
          type="button"
          aria-label="Archive"
          onClick={() => onArchive(n.id)}
          disabled={isPending}
          className={cn(
            "p-1.5 rounded-cb-1 text-t4 hover:text-t1 hover:bg-surface-3",
            "transition-colors duration-[--cb-dur] disabled:opacity-40",
          )}
        >
          <Archive size={13} strokeWidth={2} />
        </button>
      ) : (
        <button
          type="button"
          aria-label="Unarchive"
          onClick={() => onUnarchive(n.id)}
          disabled={isPending}
          className={cn(
            "p-1.5 rounded-cb-1 text-t4 hover:text-t1 hover:bg-surface-3",
            "transition-colors duration-[--cb-dur] disabled:opacity-40",
          )}
        >
          <Inbox size={13} strokeWidth={2} />
        </button>
      )}
    </div>
  </div>
);

const NotificationCenter: FC = () => {
  const [filter, setFilter] = useState<NotificationsFilter>("all");
  const { data, isFetching, isError, error, refetch } = useNotificationsQuery(filter);

  const markRead      = useMarkReadMutation();
  const archive       = useArchiveNotificationMutation();
  const unarchive     = useUnarchiveNotificationMutation();
  const markAllRead   = useMarkAllReadMutation();
  const archiveAll    = useArchiveAllMutation();

  const isMutating =
    markRead.isPending ||
    archive.isPending ||
    unarchive.isPending ||
    markAllRead.isPending ||
    archiveAll.isPending;

  const notifications = data?.data ?? [];

  return (
    <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
      {/* Header */}
      <div className="px-4 py-3 sm:px-5 border-b border-line flex flex-col sm:flex-row sm:items-center gap-3">
        <div className="flex-1">
          <h2 className="text-[14px] font-[600] text-t1">Notification center</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Your in-app notifications and action items.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => markAllRead.mutate()}
            disabled={isMutating || markAllRead.isPending}
            className={cn(
              "text-[12px] text-t3 px-2.5 py-1 rounded-cb-1 border border-line",
              "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
            )}
          >
            Mark all read
          </button>
          <button
            type="button"
            onClick={() => archiveAll.mutate()}
            disabled={isMutating || archiveAll.isPending}
            className={cn(
              "text-[12px] text-t3 px-2.5 py-1 rounded-cb-1 border border-line",
              "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              "disabled:opacity-40 disabled:cursor-not-allowed",
            )}
          >
            Archive all read
          </button>
        </div>
      </div>

      {/* Filter tabs */}
      <div className="px-4 sm:px-5 flex gap-1 overflow-x-auto pt-3 pb-0 border-b border-line">
        {FILTER_TABS.map((tab) => (
          <button
            key={tab.value}
            type="button"
            onClick={() => setFilter(tab.value)}
            className={cn(
              "shrink-0 px-3 py-1.5 text-[12.5px] rounded-t-cb-1 border border-b-0",
              "transition-colors duration-[--cb-dur]",
              filter === tab.value
                ? "bg-surface-1 border-line text-t1 font-[500]"
                : "border-transparent text-t3 hover:text-t1",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* List */}
      <div className="px-4 sm:px-5">
        {isFetching && !data ? (
          <div className="py-6 text-center">
            <p className="text-[12.5px] text-t4">Loading…</p>
          </div>
        ) : isError ? (
          <div className="py-6 flex flex-col items-center gap-2 text-center">
            <p className="text-[12.5px] text-t2">
              {error instanceof Error ? error.message : "Something went wrong"}
            </p>
            <button
              type="button"
              onClick={() => refetch()}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
                "bg-surface-1 border border-line text-[12px] text-t2",
                "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              )}
            >
              <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
              Retry
            </button>
          </div>
        ) : notifications.length === 0 ? (
          <div className="py-8 flex flex-col items-center gap-2 text-center">
            <BellOff size={20} className="text-t4" />
            <p className="text-[12.5px] text-t4">No notifications here.</p>
          </div>
        ) : (
          notifications.map((n) => (
            <NotifRow
              key={n.id}
              notification={n}
              onMarkRead={(id) => markRead.mutate(id)}
              onArchive={(id) => archive.mutate(id)}
              onUnarchive={(id) => unarchive.mutate(id)}
              isPending={isMutating}
            />
          ))
        )}
        {data && data.meta.total_pages > 1 && (
          <p className="py-3 text-[12px] text-t4 text-center">
            Showing {notifications.length} of {data.meta.total} notifications.
          </p>
        )}
      </div>
    </section>
  );
};

// ── SECTION 2: Preferences ────────────────────────────────────────────────────

interface PrefRowProps {
  item: NotificationPrefItem;
  onToggle: (params: NotificationPrefToggleParams) => void;
  isPending: boolean;
}

const PrefRow: FC<PrefRowProps> = ({ item, onToggle, isPending }) => {
  const label = item.kind === "tag"
    ? (item.tag_name ?? "Unknown tag")
    : (item.document_type_name ?? "Unknown type");

  return (
    <div className="flex items-center gap-4 py-3 border-b border-line last:border-b-0">
      <span className="flex-1 text-[13px] text-t1 truncate">{label}</span>
      <div className="flex items-center gap-4 shrink-0">
        <div className="flex flex-col items-center gap-1">
          <span className="text-[10.5px] text-t4">In-app</span>
          <Toggle
            checked={item.notify_in_app}
            onChange={(v) =>
              onToggle({
                kind: item.kind,
                ...(item.kind === "tag"
                  ? { tag_id: item.tag_id }
                  : { document_type_id: item.document_type_id }),
                notify_in_app: v,
              })
            }
            label={`In-app notifications for ${label}`}
            disabled={isPending}
          />
        </div>
        <div className="flex flex-col items-center gap-1">
          <span className="text-[10.5px] text-t4">Email</span>
          <Toggle
            checked={item.notify_email}
            onChange={(v) =>
              onToggle({
                kind: item.kind,
                ...(item.kind === "tag"
                  ? { tag_id: item.tag_id }
                  : { document_type_id: item.document_type_id }),
                notify_email: v,
              })
            }
            label={`Email notifications for ${label}`}
            disabled={isPending}
          />
        </div>
      </div>
    </div>
  );
};

const NotificationPreferences: FC = () => {
  const { data, isFetching, isError, error, refetch } =
    useNotificationPreferencesQuery();

  const toggleMutation    = useNotificationPrefToggleMutation();
  const bulkMutation      = useNotificationBulkToggleMutation();
  const digestMutation    = useDigestPreferenceMutation();
  const [digestSaved,     setDigestSaved]  = useState(false);

  const handleToggle = useCallback(
    (params: NotificationPrefToggleParams): void => {
      toggleMutation.mutate(params);
    },
    [toggleMutation],
  );

  const handleBulkToggle = useCallback(
    (kind: "tag" | "document_type", channel: "in_app" | "email", value: boolean): void => {
      bulkMutation.mutate({ kind, channel, value });
    },
    [bulkMutation],
  );

  const handleDigestToggle = useCallback(
    (value: boolean): void => {
      digestMutation.mutate(
        { email_on_waiting_on_replies_digest: value },
        {
          onSuccess: () => {
            setDigestSaved(true);
            setTimeout(() => setDigestSaved(false), 2000);
          },
        },
      );
    },
    [digestMutation],
  );

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div
        className="flex flex-col gap-3 animate-pulse"
        aria-busy="true"
      >
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-12 bg-surface-1 rounded-cb-1" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6 p-6 flex flex-col items-center gap-3 text-center">
        <Visor state="asleep" size={24} label="Error loading preferences" />
        <p className="text-[12.5px] text-t2">
          {error instanceof Error ? error.message : "Something went wrong"}
        </p>
        <button
          type="button"
          onClick={() => refetch()}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "bg-surface-1 border border-line text-[12px] text-t2",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </section>
    );
  }

  return (
    <>
      {/* Digest */}
      <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-6">
        <div className="px-4 py-3 sm:px-5 border-b border-line">
          <h2 className="text-[14px] font-[600] text-t1">Email digest</h2>
          <p className="text-[12px] text-t3 mt-0.5">
            Daily summary of emails still waiting on replies.
          </p>
        </div>
        <div className="px-4 sm:px-5">
          <div className="flex items-start gap-4 py-4">
            <div className="flex-1">
              <p className="text-[13px] font-[500] text-t1">
                Waiting-on-replies digest
              </p>
              <p className="text-[12px] text-t3 mt-0.5">
                Receive a daily email listing conversations still awaiting a reply.
              </p>
            </div>
            <div className="flex items-center gap-3 shrink-0">
              {digestSaved && (
                <span className="text-[12px] text-t3 flex items-center gap-1">
                  <Check size={12} strokeWidth={2.5} />
                  Saved
                </span>
              )}
              <Toggle
                checked={data.digest_preference}
                onChange={handleDigestToggle}
                label="Enable waiting-on-replies digest"
                disabled={digestMutation.isPending}
              />
            </div>
          </div>
          <FieldError error={digestMutation.error} />
        </div>
      </section>

      {/* Tags */}
      {data.tags.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
          <div className="px-4 py-3 sm:px-5 border-b border-line flex flex-col sm:flex-row sm:items-center gap-2">
            <div className="flex-1">
              <h2 className="text-[14px] font-[600] text-t1">Tags</h2>
              <p className="text-[12px] text-t3 mt-0.5">
                Get notified when emails with a tag arrive.
              </p>
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => handleBulkToggle("tag", "in_app", true)}
                disabled={bulkMutation.isPending}
                className={cn(
                  "text-[11.5px] text-t4 px-2 py-1 rounded-cb-1 border border-line",
                  "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                  "disabled:opacity-40 flex items-center gap-1",
                )}
              >
                <Bell size={11} />
                All in-app
              </button>
              <button
                type="button"
                onClick={() => handleBulkToggle("tag", "email", true)}
                disabled={bulkMutation.isPending}
                className={cn(
                  "text-[11.5px] text-t4 px-2 py-1 rounded-cb-1 border border-line",
                  "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                  "disabled:opacity-40 flex items-center gap-1",
                )}
              >
                <Mail size={11} />
                All email
              </button>
            </div>
          </div>
          <div className="px-4 sm:px-5">
            {data.tags.map((item) => (
              <PrefRow
                key={`tag-${item.tag_id ?? ""}`}
                item={item}
                onToggle={handleToggle}
                isPending={toggleMutation.isPending}
              />
            ))}
          </div>
        </section>
      )}

      {/* Document types */}
      {data.document_types.length > 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6">
          <div className="px-4 py-3 sm:px-5 border-b border-line flex flex-col sm:flex-row sm:items-center gap-2">
            <div className="flex-1">
              <h2 className="text-[14px] font-[600] text-t1">Document types</h2>
              <p className="text-[12px] text-t3 mt-0.5">
                Get notified when documents of a type are processed.
              </p>
            </div>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => handleBulkToggle("document_type", "in_app", true)}
                disabled={bulkMutation.isPending}
                className={cn(
                  "text-[11.5px] text-t4 px-2 py-1 rounded-cb-1 border border-line",
                  "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                  "disabled:opacity-40 flex items-center gap-1",
                )}
              >
                <Bell size={11} />
                All in-app
              </button>
              <button
                type="button"
                onClick={() => handleBulkToggle("document_type", "email", true)}
                disabled={bulkMutation.isPending}
                className={cn(
                  "text-[11.5px] text-t4 px-2 py-1 rounded-cb-1 border border-line",
                  "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                  "disabled:opacity-40 flex items-center gap-1",
                )}
              >
                <Mail size={11} />
                All email
              </button>
            </div>
          </div>
          <div className="px-4 sm:px-5">
            {data.document_types.map((item) => (
              <PrefRow
                key={`doctype-${item.document_type_id ?? ""}`}
                item={item}
                onToggle={handleToggle}
                isPending={toggleMutation.isPending}
              />
            ))}
          </div>
        </section>
      )}

      {data.tags.length === 0 && data.document_types.length === 0 && (
        <section className="bg-surface-1 cb-raised rounded-cb-2 mb-6 px-4 sm:px-5 py-8 flex flex-col items-center gap-2 text-center">
          <Bell size={20} className="text-t4" />
          <p className="text-[13px] text-t2">No tags or document types set up yet.</p>
          <p className="text-[12px] text-t4">
            Add tags and document types in Inbox settings to configure notifications.
          </p>
        </section>
      )}
    </>
  );
};

// ── NotificationsPage ─────────────────────────────────────────────────────────

export const NotificationsPage: FC = () => (
  <div data-testid="settings.page.root" className="max-w-2xl">
    <h1 className="text-[17px] font-[650] text-t1 mb-6">Notifications</h1>
    <NotificationCenter />
    <NotificationPreferences />
  </div>
);
