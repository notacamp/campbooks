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
