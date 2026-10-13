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

/** Ana's ways in: the Google account she signed up with, and a Zoho one she added. */
export const GOOGLE_WAY_IN: Schema<"WayIn"> = {
  id: "0192f1a0-0000-7000-8000-0000000000d1",
  kind: "provider",
  provider: "google",
  email: "ana@example.com",
  linked_via: "sign_up",
  linked_at: "2026-10-01T09:00:00Z",
  last_used_at: "2026-10-13T08:30:00Z",
};

export const ZOHO_WAY_IN: Schema<"WayIn"> = {
  id: "0192f1a0-0000-7000-8000-0000000000d2",
  kind: "provider",
  provider: "zoho",
  email: "ana.duarte@example.net",
  linked_via: "link",
  linked_at: "2026-10-12T15:00:00Z",
  last_used_at: null,
};

export const ALL_PROVIDERS = ["google", "microsoft", "zoho"] as const;

/** Ana's passkeys: a synced one on her Mac, and one kept on her phone. */
export const LAPTOP_PASSKEY: Schema<"PasskeyView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e1",
  name: "Passkey on Mac",
  synced: true,
  created_at: "2026-10-02T09:00:00Z",
  last_used_at: "2026-10-13T08:30:00Z",
};

export const PHONE_PASSKEY: Schema<"PasskeyView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e2",
  name: "Phone",
  synced: false,
  created_at: "2026-10-05T18:00:00Z",
  last_used_at: null,
};
