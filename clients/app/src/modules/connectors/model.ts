import { errorStatus, problemCode } from "~/lib/api";
import type { StatusTone } from "~/lib/ui";

import type { Connection } from "./api/queries";

/** The mail providers this client can name; anything else shows its raw name. */
const PROVIDERS = ["google", "zoho", "microsoft", "imap", "notion"] as const;
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
type MailboxError = (typeof LINK_ERRORS)[number] | "unknown";

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

/** The refusals of finding and connecting a mailbox with its password, by their stable code. */
const CONNECT_ERRORS = [
  "imap_login_refused",
  "smtp_login_refused",
  "imap_unreachable",
  "smtp_unreachable",
  "imap_tls_failed",
  "smtp_tls_failed",
  "imap_not_an_imap_server",
  "imap_busy",
  "host_refused",
  "starttls_missing",
  "imap_servers_required",
  "use_native_provider",
  "imap_address_invalid",
  "imap_username_invalid",
  "imap_password_invalid",
  "too_many_attempts",
  "rate_limited",
  "account_connected_by_another_member",
  "account_mismatch",
  "provider_not_configured",
  "connection_needs_user",
] as const;

/** Why finding or connecting a mailbox failed: a code the page explains, `step_up`, or `unknown`. */
export type ConnectError = (typeof CONNECT_ERRORS)[number] | "step_up" | "unknown";

/**
 * What a failed `connectors.discover_imap` or `connectors.connect_imap`
 * means, from the problem's stable `code` only (never its `title` or
 * `detail`, nor the server's own words).
 */
export function connectError(error: unknown): ConnectError {
  const code = problemCode(error);
  if (code === "step_up_required") {
    return "step_up";
  }
  const known = CONNECT_ERRORS.find((candidate) => candidate === code);
  if (known !== undefined) {
    return known;
  }
  return errorStatus(error) === 429 ? "rate_limited" : "unknown";
}
