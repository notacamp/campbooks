import type { StatusTone } from "~/lib/ui";

import type { Connection } from "./api/queries";

/** The mail providers this client can name; anything else shows its raw name. */
const PROVIDERS = ["google", "zoho", "microsoft"] as const;
export type MailProvider = (typeof PROVIDERS)[number];

export function knownMailProvider(name: string): name is MailProvider {
  return (PROVIDERS as readonly string[]).includes(name);
}

/** How a connection's health reads: a tone for the dot, and the sentence's key. */
export function healthTone(health: Connection["health"]): StatusTone {
  switch (health) {
    case "ok": {
      return "ok";
    }
    case "needs_reauth": {
      return "problem";
    }
    default: {
      return "waiting";
    }
  }
}

/** The refusals the front door and the grant intake send back (`link_error=`). */
const LINK_ERRORS = [
  "sign_in_again",
  "link_expired",
  "cancelled_at_provider",
  "login_failed",
  "provider_unavailable",
  "consent_incomplete",
  "account_mismatch",
  "account_connected_by_another_member",
  "way_in_belongs_to_another_account",
  "mailbox_unsupported",
  "provider_not_configured",
  "no_offline_access",
  "unknown_data_centre",
  "no_mailbox",
] as const;
export type MailboxError = (typeof LINK_ERRORS)[number] | "unknown";

/** How the front door came back to the Mailboxes page. */
export type MailboxResult =
  { kind: "connected"; connection: string } | { kind: "error"; error: MailboxError };

/** What the page's query says happened (`?connected=`, `?link_error=`), if anything. */
export function mailboxResult(search: {
  connected?: string | undefined;
  link_error?: string | undefined;
}): MailboxResult | undefined {
  if (search.link_error !== undefined) {
    const known = (LINK_ERRORS as readonly string[]).includes(search.link_error);
    return { kind: "error", error: known ? (search.link_error as MailboxError) : "unknown" };
  }
  return search.connected === undefined
    ? undefined
    : { kind: "connected", connection: search.connected };
}

/**
 * The row the page's keys act on first: the first mailbox that needs you
 * (`mine` says which are yours), else the first one.
 */
export function firstChoice(
  connections: readonly Connection[],
  mine: (connection: Connection) => boolean,
): string | undefined {
  const needsYou = connections.find(
    (connection) => connection.health === "needs_reauth" && mine(connection),
  );
  return (needsYou ?? connections[0])?.id;
}

/** The id `step` rows away from `current` (clamped to the list). */
export function stepChoice(
  connections: readonly Connection[],
  current: string | undefined,
  step: 1 | -1,
): string | undefined {
  if (connections.length === 0) {
    return undefined;
  }
  const index = connections.findIndex((connection) => connection.id === current);
  const next = index === -1 ? 0 : Math.min(Math.max(index + step, 0), connections.length - 1);
  return connections[next]?.id;
}
