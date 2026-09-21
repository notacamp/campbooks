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

// ── Plan ──────────────────────────────────────────────────────────────────────

export interface PlanFeatures {
  workflows: boolean;
  email_board: boolean;
  document_templates: boolean;
  email_templates: boolean;
  tasks: boolean;
  digests: boolean;
  accounting: boolean;
  imap: boolean;
  microsoft: boolean;
  [key: string]: boolean;
}

export interface PlanLimit {
  limit: number | null;
  usage: number;
  remaining: number | null;
}

export interface PlanLimits {
  members: PlanLimit;
  email_accounts: PlanLimit;
  api_clients: PlanLimit;
  [key: string]: PlanLimit;
}

/** Shape returned by Api::App::Settings::PlanController#show */
export interface PlanData {
  plan: string;
  available_plans: string[];
  self_hosted: boolean;
  features: PlanFeatures;
  limits: PlanLimits;
}

// ── Members ───────────────────────────────────────────────────────────────────

export interface MemberData {
  id: number;
  name: string | null;
  email: string;
  role: string;
  created_at: string;
}

export interface InvitedBy {
  id: number;
  name: string | null;
  email: string;
}

export interface InvitationData {
  id: number;
  email: string;
  status: string;
  admin_approved: boolean;
  expires_at: string | null;
  created_at: string;
  invited_by: InvitedBy | null;
}

/** Shape returned by Api::App::Settings::MembersController#index */
export interface MembersData {
  members: MemberData[];
  invitations: InvitationData[];
}

// ── Notifications ─────────────────────────────────────────────────────────────

export interface NotificationItem {
  id: number;
  category: string;
  priority: string;
  title: string;
  body: string | null;
  link_url: string | null;
  read: boolean;
  read_at: string | null;
  archived_at: string | null;
  resolved_at: string | null;
  active: boolean;
  archived: boolean;
  count: number;
  created_at: string;
}

export interface NotificationPrefItem {
  kind: "tag" | "document_type";
  tag_id?: number;
  tag_name?: string;
  document_type_id?: number;
  document_type_name?: string;
  notify_in_app: boolean;
  notify_email: boolean;
}

/** Shape returned by Api::App::NotificationPreferencesController#index */
export interface NotificationPreferencesData {
  digest_preference: boolean;
  tags: NotificationPrefItem[];
  document_types: NotificationPrefItem[];
}

// ── Integrations ──────────────────────────────────────────────────────────────

export interface IntegrationStatus {
  connected: boolean;
  account_count?: number;
  workspace_count?: number;
}

/** Shape returned by Api::App::Settings::Integrations::IndexController#show */
export interface IntegrationsOverviewData {
  google_drive: IntegrationStatus;
  notion: IntegrationStatus;
  zoho_drive: IntegrationStatus;
  calendars: IntegrationStatus & { account_count: number };
  connections: { count: number };
}

export interface NotionIntegration {
  id: number;
  notion_workspace_name: string | null;
  notion_workspace_id: string | null;
  created_at: string;
}

/** Shape returned by Api::App::Settings::Integrations::NotionController#show */
export interface NotionIntegrationsData {
  oauth_configured: boolean;
  integrations: NotionIntegration[];
}

export interface CalendarData {
  id: number;
  name: string;
  color: string | null;
  primary: boolean | null;
  syncing: boolean;
}

export interface CalendarAccountData {
  id: number;
  provider: string;
  email: string | null;
  active: boolean;
  calendar_count: number;
  calendars: CalendarData[];
}

/** Shape returned by Api::App::Settings::Integrations::CalendarsController#show */
export interface CalendarsData {
  accounts: CalendarAccountData[];
}

export interface ConnectionData {
  id: number;
  name: string;
  base_url: string;
  auth_type: string;
  auth_header_name: string | null;
  auth_username: string | null;
  has_secret: boolean;
  created_at: string;
}
