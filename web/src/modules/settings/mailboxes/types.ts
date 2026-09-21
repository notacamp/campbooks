/**
 * modules/settings/mailboxes/types — wire types for the mailbox connect surface.
 *
 * Snake_case field names mirror the Rails Api::App::Email::AccountSerializer
 * exactly. Do NOT rename to camelCase here.
 */

// ── Connected account ─────────────────────────────────────────────────────────

export type MailboxProvider = "google" | "zoho" | "microsoft" | "imap";

export interface ConnectedMailbox {
  id: number;
  email_address: string;
  display_name: string;
  provider: MailboxProvider;
  active: boolean;
  scanning: boolean;
  last_scanned_at: string | null;
  push_watch_expires_at: string | null;
  can_read: boolean;
  can_send: boolean;
  can_manage: boolean;
  is_owner: boolean;
}

// ── OAuth authorize URL ───────────────────────────────────────────────────────

export interface AuthorizeUrlResponse {
  provider: MailboxProvider;
  authorize_url: string;
  note: string | null;
}

// ── IMAP connect ──────────────────────────────────────────────────────────────

export interface ImapConnectParams {
  email_address: string;
  password: string;
  imap_host: string;
  imap_port: number;
  imap_ssl: boolean;
  smtp_host: string;
  smtp_port: number;
  smtp_ssl: boolean;
}
