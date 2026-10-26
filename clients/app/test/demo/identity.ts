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
  offers: ["calendar"],
  health: "ok",
  health_changed_at: "2026-10-13T09:00:00Z",
  health_reason: null,
  granted_by_you: true,
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
  mailboxes: 0,
  connections: [],
};

export const ZOHO_WAY_IN: Schema<"WayIn"> = {
  id: "0192f1a0-0000-7000-8000-0000000000d2",
  kind: "provider",
  provider: "zoho",
  email: "ana.duarte@example.net",
  linked_via: "link",
  linked_at: "2026-10-12T15:00:00Z",
  last_used_at: null,
  mailboxes: 0,
  connections: [],
};

export const ALL_PROVIDERS = ["google", "microsoft", "zoho"] as const;

/** Ana's Zoho Mail mailbox in Duarte & Pessoa, waiting for her to sign in again. */
export const ZOHO_CONNECTION: Schema<"ConnectionView"> = {
  ...CONNECTION,
  id: "0192f1a0-0000-7000-8000-0000000000c2",
  provider: "zoho",
  address: "ana.duarte@example.net",
  // Zoho Calendar is offered only while the server's adapter is on.
  offers: [],
  health: "needs_reauth",
  health_reason: "grant_revoked",
};

/** The shop's shared mailbox Ana connected without signing in with it. */
export const SHARED_CONNECTION: Schema<"ConnectionView"> = {
  ...CONNECTION,
  id: "0192f1a0-0000-7000-8000-0000000000c4",
  address: "loja@example.com",
  display_name: "Loja Duarte & Pessoa",
};

/** Sofia's Gmail mailbox, paused by Gmail for a few minutes. */
export const SOFIA_CONNECTION: Schema<"ConnectionView"> = {
  ...CONNECTION,
  id: "0192f1a0-0000-7000-8000-0000000000c3",
  address: "sofia@example.com",
  display_name: "Sofia Pessoa",
  granted_by: "0192f1a0-0000-7000-8000-0000000000a2",
  granted_by_you: false,
  health: "throttled",
  health_reason: "rate_limited",
};

/**
 * The shop's mailbox at its hosting company, connected over IMAP with a
 * username that is not its address: its address is being checked.
 */
export const IMAP_CONNECTION: Schema<"ConnectionView"> = {
  ...CONNECTION,
  id: "0192f1a0-0000-7000-8000-0000000000c5",
  provider: "imap",
  offers: [],
  address: "ana@bakery.example",
  address_verified: false,
  address_check: "checking",
  display_name: null,
};

/** Discovery for a Yahoo address: Yahoo's servers, and an app password. */
export const YAHOO_DISCOVERY: Schema<"DiscoveredImap"> = {
  kind: "settings",
  imap: { host: "imap.mail.yahoo.com", port: 993, security: "tls" },
  smtp: { host: "smtp.mail.yahoo.com", port: 465, security: "tls" },
  username_hint: "ana@yahoo.example",
  source: "ispdb",
  provider_hint: "yahoo.com",
  password: "app_password",
};

/** Discovery for a GMX address (its German site): GMX's servers, and an app password. */
export const GMX_DISCOVERY: Schema<"DiscoveredImap"> = {
  kind: "settings",
  imap: { host: "imap.gmx.net", port: 993, security: "tls" },
  smtp: { host: "mail.gmx.net", port: 587, security: "starttls" },
  username_hint: "ana@gmx.example",
  source: "ispdb",
  provider_hint: "gmx.net",
  password: "app_password",
};

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

/**
 * An instance app (registered by this server's administrator from its CLI):
 * Applicamp, signing Ana in on an https callback.
 */
export const INSTANCE_APP_CONSENT: Schema<"ConsentRequest"> = {
  client: {
    id: "app_Z3k8rT1qLm0vXc5bN2yW4a",
    name: "Applicamp",
    kind: "instance",
    verified: false,
    workspace_name: null,
    logo_uri: null,
    redirect_host: "applicamp.example.com",
    document_host: null,
    device: false,
  },
  scopes: [
    { name: "openid", description: "Know who you are on Campbooks." },
    { name: "profile", description: "See your name." },
    { name: "email", description: "See your email address." },
    {
      name: "offline_access",
      description: "Keep access when you are not using it, until you revoke it.",
    },
    { name: "conversations:read", description: "Read your conversations." },
    { name: "schedule:read", description: "See your calendars." },
  ],
  binds_workspace: true,
  workspaces: [
    { id: "0192f1a0-0000-7000-8000-000000000001", name: "Duarte & Pessoa", role: "owner" },
    { id: "0192f1a0-0000-7000-8000-000000000002", name: "Household", role: "member" },
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

/** Ana's Notion, connected for Acme's pages (Notion gives no address; the connection is named for its workspace). */
export const NOTION_CONNECTION: Schema<"ConnectionView"> = {
  ...CONNECTION,
  id: "0192f1a0-0000-7000-8000-0000000000c9",
  provider: "notion",
  address: "",
  address_verified: false,
  display_name: "Acme",
  capabilities: ["pages"],
  offers: [],
};
