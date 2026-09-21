/**
 * modules/inbox/types — shared types for the Inbox module.
 *
 * Row shape mirrors Api::App::PeopleStandingSerializer output (snake_case,
 * as the Rails API sends it). Do NOT rename to camelCase here — the
 * serializer wire format is authoritative.
 */

// ── Row type ──────────────────────────────────────────────────────────────────

/** A single row returned by GET /api/app/people. */
export interface InboxRow {
  id: number;
  /** "Contact" | "Organization" */
  counterpart_type: string;
  name: string;
  subtitle: string | null;
  avatar_initial: string | null;
  avatar_email: string | null;
  needs_you: boolean;
  /** reply | decide | do | pay | chase | nudge */
  verb: string | null;
  stand_line: string | null;
  wait_days: number | null;
  unread: boolean;
  score: number;
  last_activity_at: string | null;
  email_message_id: number | null;
  feed_item_id: number | null;
  standing_kind: string | null;
}

// ── Tab type ──────────────────────────────────────────────────────────────────

/** Inbox tab maps to the API `tab` parameter. */
export type InboxTab = "needing" | "latest";

/** Display label for each tab. */
export const TAB_LABELS: Record<InboxTab, string> = {
  needing: "Important",
  latest: "Everything",
};

// ── Archive action ────────────────────────────────────────────────────────────

export interface ArchiveAction {
  kind: "archive" | "unarchive";
}

// ── Optimistic snapshot type ──────────────────────────────────────────────────

/** Infinite query data shape for rollback in optimistic updates. */
export interface InboxQueryData {
  pages: Array<{ data: InboxRow[]; meta: { page: number; per_page: number; total: number; total_pages: number } }>;
  pageParams: unknown[];
}
