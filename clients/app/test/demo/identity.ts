import type { Schema } from "~/lib/api";

// Fictional demo data for the sign-in screens' tests and stories (never a real person).
export const REQUEST = "req-demo-1";

export const NEW_ACCOUNT = {
  provider: "google",
  email: "ana@example.com",
  display_name: "Ana Duarte",
} as const;

export const HELD_GRANT = {
  id: "0192f1a0-0000-7000-8000-0000000000b1",
  provider: "google",
  address: "ana@example.com",
  display_name: "Ana Duarte",
  expires_at: "2026-10-20T09:00:00Z",
} as const;

export const CONNECTION: Schema<"ConnectionView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000c1",
  provider: "google",
  address: "ana@example.com",
  address_verified: true,
  display_name: "Ana Duarte",
  granted_by: "0192f1a0-0000-7000-8000-0000000000a1",
  capabilities: ["messages"],
  health: "ok",
  health_changed_at: "2026-10-13T09:00:00Z",
  health_reason: null,
  streams: [],
  created_at: "2026-10-13T09:00:00Z",
};
