// Fictional demo data for the admin section's tests and stories.
import type { AdminSchema } from "~/lib/api";

type AdminOverview = AdminSchema<"AdminOverview">;

/** The system overview of a small self-hosted instance (admin API). */
export const OVERVIEW: AdminOverview = {
  version: "0.1.0",
  commit: "3f2c1e0",
  started_at: "2026-10-11T08:30:00Z",
  uptime_seconds: 2 * 86_400 + 5 * 3600 + 12 * 60,
  database: {
    reachable: true,
    latency_ms: 3,
    migrations_applied: 18,
    latest_migration: 20_261_015_093_100,
    latest_migration_description: "admin init",
  },
  configuration: {
    environment: "production",
    public_origin: "https://campbooks.example.com",
    signup_policy: "approval",
    blob_store: "s3",
    database_logins: "split",
    migrations: "on_start",
    serves_web_client: true,
    exports_telemetry: false,
    metrics_endpoint: true,
  },
  services: {
    identity_provider: "built_in",
    sign_in_providers: ["google"],
    mail_providers: ["google"],
  },
  ai: [
    {
      role: "decide",
      configured: true,
      model: "qwen3:8b",
      endpoint: "http://ollama:11434",
      protocol: "openai",
      reachable: true,
    },
    {
      role: "embed",
      configured: true,
      model: "BAAI/bge-m3",
      endpoint: "http://tei:8080",
      protocol: "openai",
      reachable: false,
    },
    {
      role: "complete",
      configured: false,
      model: null,
      endpoint: null,
      protocol: null,
      reachable: null,
    },
    {
      role: "vision",
      configured: false,
      model: null,
      endpoint: null,
      protocol: null,
      reachable: null,
    },
  ],
};

/** The same instance with no AI role set up. */
export const OVERVIEW_WITHOUT_AI: AdminOverview = {
  ...OVERVIEW,
  ai: OVERVIEW.ai.map((role) => ({
    role: role.role,
    configured: false,
    model: null,
    endpoint: null,
    protocol: null,
    reachable: null,
  })),
};

/** What the admin API answers a stale sign-in. */
export const REAUTH_PROBLEM = {
  type: "about:blank",
  title: "Unauthorized",
  status: 401,
  detail: "For your security, administration needs a recent sign-in. Sign in again to continue.",
  code: "admin_reauth_required",
} as const;

/** What the admin API answers everyone who is not an administrator. */
export const NOT_FOUND_PROBLEM = {
  type: "about:blank",
  title: "Not found",
  status: 404,
  detail: "This resource does not exist, or you cannot see it.",
  code: "not_found",
} as const;

// --- People and Workspaces (slice S2) ------------------------------------------

type AdminUser = AdminSchema<"AdminUser">;
type AdminUserDetail = AdminSchema<"AdminUserDetail">;
type AdminWorkspace = AdminSchema<"AdminWorkspace">;
type AdminWorkspaceDetail = AdminSchema<"AdminWorkspaceDetail">;
type AdminAccount = AdminSchema<"AdminAccount">;
type AdminConnection = AdminSchema<"AdminConnection">;

/** Ana, an owner of two workspaces (fictional). */
export const ADMIN_ANA: AdminUser = {
  id: "0192f1a0-0000-7000-8000-0000000000a1",
  account_id: "0192f1a0-0000-7000-8000-0000000000c1",
  display_name: "Ana Duarte",
  email: "ana@example.com",
  created_at: "2026-09-01T09:00:00Z",
  workspace_count: 2,
};

/** Rui, a member of one workspace. */
export const ADMIN_RUI: AdminUser = {
  id: "0192f1a0-0000-7000-8000-0000000000a3",
  account_id: "0192f1a0-0000-7000-8000-0000000000c3",
  display_name: "Rui Matos",
  email: "rui@example.com",
  created_at: "2026-09-10T09:00:00Z",
  workspace_count: 1,
};

/** Duarte & Pessoa, as an administrator sees it. */
export const ADMIN_DUARTE: AdminWorkspace = {
  id: "0192f1a0-0000-7000-8000-000000000001",
  name: "Duarte & Pessoa",
  kind: "business",
  created_at: "2026-09-01T09:00:00Z",
  deleted_at: null,
  member_count: 2,
  owner_ids: [ADMIN_ANA.id],
};

/** A personal workspace being deleted. */
export const ADMIN_OLD_HOUSEHOLD: AdminWorkspace = {
  id: "0192f1a0-0000-7000-8000-000000000003",
  name: "Old household",
  kind: "personal",
  created_at: "2026-08-01T09:00:00Z",
  deleted_at: "2026-10-01T09:00:00Z",
  member_count: 1,
  owner_ids: [ADMIN_ANA.id],
};

/** Ana's page: her workspaces and roles. */
export const ADMIN_ANA_DETAIL: AdminUserDetail = {
  user: ADMIN_ANA,
  workspaces: [
    {
      id: ADMIN_DUARTE.id,
      name: ADMIN_DUARTE.name,
      kind: "business",
      role: "owner",
      joined_at: "2026-09-01T09:00:00Z",
      deleted_at: null,
    },
    {
      id: ADMIN_OLD_HOUSEHOLD.id,
      name: ADMIN_OLD_HOUSEHOLD.name,
      kind: "personal",
      role: "owner",
      joined_at: "2026-08-01T09:00:00Z",
      deleted_at: "2026-10-01T09:00:00Z",
    },
  ],
};

/** Duarte & Pessoa's page: its members and one pending invitation. */
export const ADMIN_DUARTE_DETAIL: AdminWorkspaceDetail = {
  workspace: ADMIN_DUARTE,
  members: [
    {
      user_id: ADMIN_ANA.id,
      display_name: "Ana Duarte",
      email: "ana@example.com",
      role: "owner",
      joined_at: "2026-09-01T09:00:00Z",
    },
    {
      user_id: ADMIN_RUI.id,
      display_name: "Rui Matos",
      email: "rui@example.com",
      role: "member",
      joined_at: "2026-09-10T09:00:00Z",
    },
  ],
  pending_invitations: 1,
};

/** Ana's identity account: Google, a passkey, signed in on two browsers. */
export const ADMIN_ANA_ACCOUNT: AdminAccount = {
  id: ADMIN_ANA.account_id,
  display_name: "Ana Duarte",
  primary_email: "ana@example.com",
  primary_email_verified: true,
  created_at: "2026-09-01T08:59:00Z",
  suspended_at: null,
  ways_in: [
    {
      provider: "google",
      email: "ana@example.com",
      linked_via: "sign_up",
      linked_at: "2026-09-01T08:59:00Z",
      last_used_at: "2026-10-12T08:00:00Z",
    },
  ],
  passkeys: 1,
  sessions: { active: 2, last_sign_in_at: "2026-10-12T08:00:00Z" },
};

/** The same account, suspended. */
export const ADMIN_ANA_SUSPENDED: AdminAccount = {
  ...ADMIN_ANA_ACCOUNT,
  suspended_at: "2026-10-13T10:00:00Z",
  sessions: { active: 0, last_sign_in_at: "2026-10-12T08:00:00Z" },
};

/** Duarte & Pessoa's mailboxes: one syncing, one whose grant was revoked. */
export const ADMIN_CONNECTIONS: AdminConnection[] = [
  {
    id: "0192f1a0-0000-7000-8000-0000000000d1",
    provider: "google",
    address: "sales@example.com",
    granted_by: ADMIN_ANA.id,
    capabilities: ["messages"],
    health: "ok",
    health_reason: null,
    health_changed_at: "2026-09-01T09:05:00Z",
    created_at: "2026-09-01T09:05:00Z",
    streams: [
      {
        capability: "messages",
        phase: "incremental",
        paused: false,
        last_synced_at: "2026-10-13T09:58:00Z",
        consecutive_failures: 0,
      },
    ],
  },
  {
    id: "0192f1a0-0000-7000-8000-0000000000d2",
    provider: "google",
    address: "billing@example.com",
    granted_by: ADMIN_RUI.id,
    capabilities: ["messages"],
    health: "needs_reauth",
    health_reason: "grant_revoked",
    health_changed_at: "2026-10-12T16:00:00Z",
    created_at: "2026-09-10T09:05:00Z",
    streams: [
      {
        capability: "messages",
        phase: "incremental",
        paused: true,
        last_synced_at: "2026-10-12T15:58:00Z",
        consecutive_failures: 3,
      },
    ],
  },
];
