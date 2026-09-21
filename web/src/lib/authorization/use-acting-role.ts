/**
 * lib/authorization — abilities and acting-role stub.
 *
 * The pattern (from connect-web-v2):
 *   1. useActingRole() returns the current user's role with typed ability methods.
 *   2. Module abilities live in modules/<name>/abilities/ — one file per resource,
 *      named for the Rails policy it mirrors (e.g. abilities/accessEmail.ts mirrors
 *      access_email_policy.rb). Abilities call role.can() / role.canAny().
 *   3. A forbidden action renders NOTHING — no disabled control, no error state,
 *      just no element. Entitlements (plan-level gates) are a separate axis.
 *
 * TODO: wire useActingRole to GET /api/app/me (or a dedicated /api/app/role
 * endpoint) via useQuery once the auth module lands.
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";

// ── Types ─────────────────────────────────────────────────────────────────────

export type Permission =
  | "email:read"
  | "email:write"
  | "email:send"
  | "document:read"
  | "document:write"
  | "contact:read"
  | "contact:write"
  | "calendar:read"
  | "calendar:write"
  | "money:read"
  | "money:write"
  | "settings:read"
  | "settings:write"
  | "workspace:admin";

export interface ActingRole {
  /** The role type string returned by the API. */
  type: string | null;
  /** True once the role has loaded (or definitively failed). */
  isResolved: boolean;
  /** Returns true when the role grants ALL listed permissions. */
  can: (...permissions: Permission[]) => boolean;
  /** Returns true when the role grants ANY of the listed permissions. */
  canAny: (...permissions: Permission[]) => boolean;
}

interface RoleResponse {
  type: string;
  permissions: Permission[];
}

// ── Resolved role factory ─────────────────────────────────────────────────────

const makeRole = (data: RoleResponse | null): ActingRole => ({
  type: data?.type ?? null,
  isResolved: true,
  can: (...permissions) =>
    permissions.every((p) => data?.permissions.includes(p) ?? false),
  canAny: (...permissions) =>
    permissions.some((p) => data?.permissions.includes(p) ?? false),
});

/** An unresolved role that grants nothing — used while the query is loading. */
const UNRESOLVED_ROLE: ActingRole = {
  type: null,
  isResolved: false,
  can: () => false,
  canAny: () => false,
};

// ── Hook ──────────────────────────────────────────────────────────────────────

/**
 * Returns the acting user's role and typed ability methods.
 * Unresolved role grants nothing — gates are closed while loading.
 */
export const useActingRole = (): ActingRole => {
  const result: UseQueryResult<RoleResponse> = useQuery<RoleResponse>({
    queryKey: ["acting-role"],
    queryFn: () => apiClient<RoleResponse>("/me/role"),
    retry: false,
  });

  if (!result.isSuccess || !result.data) return UNRESOLVED_ROLE;
  return makeRole(result.data);
};
