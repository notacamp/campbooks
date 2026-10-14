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

/** A request an MCP agent made: unverified, on this device, bound to a workspace. */
export const AGENT_CONSENT: Schema<"ConsentRequest"> = {
  client: {
    id: "dcr_8bW2YhQ1k3mJxV0aT6pZ",
    name: "Desk Agent",
    kind: "dynamic",
    verified: false,
    workspace_name: null,
    logo_uri: null,
    redirect_host: "127.0.0.1",
    document_host: null,
    device: true,
  },
  scopes: [
    { name: "conversations:read", description: "Read your conversations." },
    { name: "offline_access", description: "Keep access when you are not using it." },
  ],
  binds_workspace: true,
  workspaces: [
    { id: "0192f1a0-0000-7000-8000-000000000001", name: "Duarte & Pessoa", role: "owner" },
    { id: "0192f1a0-0000-7000-8000-000000000002", name: "Household", role: "member" },
  ],
  account_name: "Ana Duarte",
  account_email: "ana@example.com",
  remembered: false,
};

/** Campbooks' own desktop app signing in: asked every time, every workspace. */
export const NATIVE_CONSENT: Schema<"ConsentRequest"> = {
  client: {
    id: "campbooks-native",
    name: "Campbooks for desktop and mobile",
    kind: "first_party",
    verified: true,
    workspace_name: null,
    logo_uri: null,
    redirect_host: "127.0.0.1",
    document_host: null,
    device: true,
  },
  scopes: [
    { name: "openid", description: "Know who you are on Campbooks." },
    { name: "conversations:read", description: "Read your conversations." },
    { name: "conversations:write", description: "Reply, send and sort your conversations." },
  ],
  binds_workspace: false,
  workspaces: [],
  account_name: "Ana Duarte",
  account_email: "ana@example.com",
  remembered: false,
};

/** A workspace's own sign-in app with an https callback: remembered once allowed. */
export const TEAM_APP_CONSENT: Schema<"ConsentRequest"> = {
  client: {
    id: "cb_Qm4u2c0k3t9xWZ1yB7aE",
    name: "Studio dashboard",
    kind: "workspace",
    verified: false,
    workspace_name: "Duarte & Pessoa",
    logo_uri: null,
    redirect_host: "dashboard.example.com",
    document_host: null,
    device: false,
  },
  scopes: [{ name: "conversations:read", description: "Read your conversations." }],
  binds_workspace: true,
  workspaces: [
    { id: "0192f1a0-0000-7000-8000-000000000001", name: "Duarte & Pessoa", role: "owner" },
  ],
  account_name: "Ana Duarte",
  account_email: "ana@example.com",
  remembered: true,
};

/** Ana's connected apps: an agent in Duarte & Pessoa, and the desktop app. */
export const CONNECTED_APPS: Schema<"ConnectedApp">[] = [
  {
    id: "0192f1a0-0000-7000-8000-0000000000e1",
    client_id: "dcr_8bW2YhQ1k3mJxV0aT6pZ",
    client_name: "Desk Agent",
    client_kind: "dynamic",
    verified: false,
    workspace_id: "0192f1a0-0000-7000-8000-000000000001",
    scopes: ["conversations:read", "offline_access"],
    granted_at: "2026-10-12T10:00:00Z",
    last_used_at: "2026-10-13T08:00:00Z",
  },
  {
    id: "0192f1a0-0000-7000-8000-0000000000e2",
    client_id: "campbooks-native",
    client_name: "Campbooks for desktop and mobile",
    client_kind: "first_party",
    verified: true,
    workspace_id: null,
    scopes: ["openid", "conversations:read", "conversations:write"],
    granted_at: "2026-10-10T09:00:00Z",
    last_used_at: null,
  },
];

/** Duarte & Pessoa's own apps: a nightly export with a secret. */
export const WORKSPACE_CLIENTS: Schema<"WorkspaceClient">[] = [
  {
    client_id: "cb_3nV8sQ2pL0aM9xK7tR1w",
    name: "Nightly export",
    type: "confidential",
    scopes: ["conversations:read"],
    redirect_uris: [],
    created_at: "2026-10-11T09:00:00Z",
    secret_rotated_at: null,
    last_used_at: "2026-10-13T02:00:00Z",
  },
];

/** A secret shown once (fictional). */
export const DEMO_SECRET = "demo-secret-3c9f0b7a1e5d4c2b8a6f0e1d2c3b4a59";

/** Ana's passkeys: a synced one on her Mac, and one kept on her phone. */
export const LAPTOP_PASSKEY: Schema<"PasskeyView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e1",
  name: "Passkey on Mac",
  synced: true,
  created_at: "2026-10-02T09:00:00Z",
  last_used_at: "2026-10-13T08:30:00Z",
  signed_up_with: false,
};

export const PHONE_PASSKEY: Schema<"PasskeyView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e2",
  name: "Phone",
  synced: false,
  created_at: "2026-10-05T18:00:00Z",
  last_used_at: null,
  signed_up_with: false,
};

/** The passkey a passkey-only account was made with, two days ago in the demo week. */
export const SIGN_UP_PASSKEY: Schema<"PasskeyView"> = {
  id: "0192f1a0-0000-7000-8000-0000000000e3",
  name: "Passkey",
  synced: true,
  created_at: "2026-10-12T10:00:00Z",
  last_used_at: "2026-10-13T08:30:00Z",
  signed_up_with: true,
};
