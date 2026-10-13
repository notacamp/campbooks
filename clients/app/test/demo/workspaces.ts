// Fictional demo data for tests and stories (never a real person).
export const DUARTE = {
  id: "0192f1a0-0000-7000-8000-000000000001",
  name: "Duarte & Pessoa",
  kind: "business",
  accent: "business",
  role: "owner",
  created_at: "2026-09-01T09:00:00Z",
} as const;

export const HOUSEHOLD = {
  id: "0192f1a0-0000-7000-8000-000000000002",
  name: "Household",
  kind: "personal",
  accent: "personal",
  role: "member",
  created_at: "2026-09-02T09:00:00Z",
} as const;

export const ANA = {
  user_id: "0192f1a0-0000-7000-8000-0000000000a1",
  display_name: "Ana Duarte",
  email: "ana@example.com",
  role: "owner",
  joined_at: "2026-09-01T09:00:00Z",
} as const;

export const SOFIA = {
  user_id: "0192f1a0-0000-7000-8000-0000000000a2",
  display_name: "Sofia Pessoa",
  email: "sofia@example.com",
  role: "admin",
  joined_at: "2026-09-03T09:00:00Z",
} as const;

export const RUI = {
  user_id: "0192f1a0-0000-7000-8000-0000000000a3",
  display_name: "Rui Matos",
  email: "rui@example.com",
  role: "member",
  joined_at: "2026-09-10T09:00:00Z",
} as const;

export const PENDING = {
  id: "0192f1a0-0000-7000-8000-0000000000b1",
  email: "marta@example.com",
  role: "member",
  invited_by: ANA.user_id,
  created_at: "2026-10-10T09:00:00Z",
  expires_at: "2099-10-17T09:00:00Z",
} as const;

export const EXPIRED = {
  id: "0192f1a0-0000-7000-8000-0000000000b2",
  email: "joao@example.com",
  role: "admin",
  invited_by: ANA.user_id,
  created_at: "2026-09-01T09:00:00Z",
  expires_at: "2026-09-08T09:00:00Z",
} as const;

export const SESSION = {
  authenticated: true,
  user: { sub: "idp-ana", name: ANA.display_name, email: ANA.email },
  expires_at: "2099-01-01T00:00:00Z",
} as const;
