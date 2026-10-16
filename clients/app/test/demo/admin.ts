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
      location: "instance",
      content_allowed: true,
      reachable: true,
      answers_check: {
        state: "passed",
        right: 33,
        total: 33,
        checked_at: "2026-10-14T06:00:00Z",
      },
    },
    {
      role: "embed",
      configured: true,
      model: "BAAI/bge-m3",
      endpoint: "http://tei:8080",
      protocol: "openai",
      location: "instance",
      content_allowed: true,
      reachable: false,
    },
    {
      role: "complete",
      configured: true,
      model: "open-mixtral",
      endpoint: "https://api.example.com",
      protocol: "openai",
      location: "external",
      content_allowed: false,
      reachable: true,
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
      you: false,
    },
    {
      user_id: ADMIN_RUI.id,
      display_name: "Rui Matos",
      email: "rui@example.com",
      role: "member",
      joined_at: "2026-09-10T09:00:00Z",
      you: false,
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

type SignupRequest = AdminSchema<"SignupRequestView">;
type SignupCode = AdminSchema<"SignupCodeView">;

/** Two people waiting and one turned down (fictional). */
export const SIGNUP_REQUESTS: SignupRequest[] = [
  {
    user_id: "0192f1a0-0000-7000-8000-0000000000a3",
    display_name: "Inês Barros",
    email: "ines@example.com",
    status: "pending",
    requested_at: "2026-10-12T09:30:00Z",
    decided_at: null,
    decided_by: null,
    reason: null,
  },
  {
    user_id: "0192f1a0-0000-7000-8000-0000000000a2",
    display_name: null,
    email: "tomas@example.com",
    status: "pending",
    requested_at: "2026-10-11T18:05:00Z",
    decided_at: null,
    decided_by: null,
    reason: null,
  },
  {
    user_id: "0192f1a0-0000-7000-8000-0000000000a1",
    display_name: "Rui Matos",
    email: "rui@example.com",
    status: "rejected",
    requested_at: "2026-10-09T07:00:00Z",
    decided_at: "2026-10-10T10:00:00Z",
    decided_by: "0192f1a0-0000-7000-8000-0000000000ff",
    reason: "Not a customer",
  },
];

/** An active single-use code, a team code half used, and a spent one. */
export const SIGNUP_CODES: SignupCode[] = [
  {
    id: "0192f1a0-0000-7000-8000-0000000000c3",
    label: "Beta, Inês",
    state: "active",
    max_uses: 1,
    uses: 0,
    expires_at: null,
    created_by: "0192f1a0-0000-7000-8000-0000000000ff",
    created_at: "2026-10-12T10:00:00Z",
    revoked_at: null,
  },
  {
    id: "0192f1a0-0000-7000-8000-0000000000c2",
    label: "Team offsite",
    state: "active",
    max_uses: 5,
    uses: 2,
    expires_at: "2026-11-30T22:59:59Z",
    created_by: "0192f1a0-0000-7000-8000-0000000000ff",
    created_at: "2026-10-10T10:00:00Z",
    revoked_at: null,
  },
  {
    id: "0192f1a0-0000-7000-8000-0000000000c1",
    label: "Launch day",
    state: "used_up",
    max_uses: 1,
    uses: 1,
    expires_at: null,
    created_by: "0192f1a0-0000-7000-8000-0000000000ff",
    created_at: "2026-10-01T10:00:00Z",
    revoked_at: null,
  },
];

// --- Jobs (slice S3) ---------------------------------------------------------

type QueueSummary = AdminSchema<"QueueSummary">;
type FailedJob = AdminSchema<"FailedJob">;
type FailedPage = AdminSchema<"FailedPage">;

/** The job queue of a small instance: a busy sync, an AI read that keeps failing, a lost worker. */
export const JOBS_SUMMARY: QueueSummary = {
  at: "2026-10-13T09:30:00Z",
  oldest_ready_seconds: 190,
  kinds: [
    {
      queue: "ai",
      kind: "conversations.read_message",
      ready: 12,
      scheduled: 3,
      running: 2,
      expired_leases: 0,
      failed: 2,
      oldest_ready_seconds: 190,
    },
    {
      queue: "default",
      kind: "system.prune_jobs",
      ready: 0,
      scheduled: 1,
      running: 0,
      expired_leases: 0,
      failed: 0,
      oldest_ready_seconds: null,
    },
    {
      queue: "sync",
      kind: "connectors.sync_mailbox",
      ready: 1,
      scheduled: 0,
      running: 1,
      expired_leases: 1,
      failed: 0,
      oldest_ready_seconds: 20,
    },
  ],
};

/** Nothing queued at all. */
export const JOBS_SUMMARY_EMPTY: QueueSummary = {
  at: "2026-10-13T09:30:00Z",
  oldest_ready_seconds: null,
  kinds: [],
};

const failedJob = (n: number, overrides: Partial<FailedJob> = {}): FailedJob => ({
  id: `0199f0a0-0000-7000-8000-00000000000${String(n)}`,
  queue: "ai",
  kind: "conversations.read_message",
  workspace_id: "wsp_demo",
  attempts: 10,
  max_attempts: 10,
  last_error: "provider returned 503: upstream timed out while reading the message",
  created_at: "2026-10-13T06:00:00Z",
  finished_at: "2026-10-13T08:15:00Z",
  ...overrides,
});

/** Two dead jobs: one with a long multi-line error, one with none recorded. */
export const FAILED_JOBS: FailedPage = {
  items: [
    failedJob(1, {
      last_error: `model returned an invalid decision: missing field \`reply\` at line 1 column 57\n${"  at conversations::read::decide ".repeat(8)}`,
    }),
    failedJob(2, { workspace_id: null, last_error: null, attempts: 1, max_attempts: 1 }),
  ],
  next: null,
};

/** The first page of many. */
export const FAILED_JOBS_FIRST_PAGE: FailedPage = {
  items: [failedJob(3)],
  next: "1760344500000000.0199f0a0-0000-7000-8000-000000000003",
};
