/**
 * modules/settings/types — TypeScript shapes for the Settings API responses.
 *
 * These match the serializer/controller wire format exactly (snake_case fields).
 * Import from "~/modules/settings" rather than this file directly.
 */

// ── Account ───────────────────────────────────────────────────────────────────

/** Shape returned by Api::App::Settings::AccountSerializer */
export interface AccountData {
  id: number;
  name: string | null;
  email: string;
  locale: string;
  time_zone: string | null;
  role: string;
  compose_default: string | null;
  writing_style: string | null;
  writing_style_learned: boolean;
  writing_style_updated_at: string | null;
  deletion_requested_at: string | null;
  created_at: string;
}

// ── Workspace ─────────────────────────────────────────────────────────────────

/** Shape returned by Api::App::Settings::WorkspaceController#workspace_data */
export interface WorkspaceData {
  id: number;
  name: string;
  slug: string;
  app_name: string;
  plan: string;
  workspace_context: string | null;
  company_nif: string | null;
  self_hosted: boolean;
  created_at: string;
}

// ── Data privacy ──────────────────────────────────────────────────────────────

/** Shape returned by Api::App::Settings::DataPrivacyController#privacy_data */
export interface PrivacyData {
  ai_processing_enabled: boolean;
  scout_thread_posts: boolean | null;
  required_data_region: string | null;
  email_retention_months: number | null;
  google_drive_connected: boolean;
  notion_connected: boolean;
  zoho_drive_connected: boolean;
  calendar_connected: boolean;
}
