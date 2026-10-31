// The reasons a draft or a save can say in words (`compose.errors.*`): a
// send the connector refused, a scheduled send that stopped
// (campbooks-cloud#392) and a save the server refused.

/** The reasons with a message of their own; anything else says "something went wrong". */
export const KNOWN_ERRORS: ReadonlySet<string> = new Set([
  "not_saved",
  "content_changed",
  "no_recipients",
  "invalid_address",
  "attachments_too_large",
  "send_at_invalid",
  "send_at_busy",
  "inbox_read_only",
  "switch_failed",
  "draft_not_editable",
  "not_connected",
  "needs_reauth",
  "rejected",
  "too_large",
  "unavailable",
  "unknown_outcome",
  // A scheduled send that stopped:
  "destination_changed",
  "sender_may_not_send",
  "release_lost",
]);

/** The `compose.errors` key that says `reason`. */
export function errorKey(reason: string): string {
  return `compose.errors.${KNOWN_ERRORS.has(reason) ? reason : "unknown"}`;
}
