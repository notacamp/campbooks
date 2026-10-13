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
