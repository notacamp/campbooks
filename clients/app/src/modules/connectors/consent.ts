/**
 * The front door, as the Mailboxes page sees it: a port the app shell fills
 * with `identity`'s implementation (the client mirror of the kernel's
 * `ConsentDoor`, which `identity` implements for `connectors` on the
 * server). `connectors` cannot import `identity`, which imports it.
 */

/**
 * What to start: connecting a new mailbox (`signIn`: also sign in with that
 * account; `false` for a shared mailbox), or reconnecting one of yours.
 */
export type ConsentStart =
  | { purpose: "connect"; provider: string; signIn: boolean }
  | { purpose: "reconnect"; connection: string };

/**
 * What disconnecting a mailbox does to the way in of its account:
 * `removed` when it was added only by connecting it and is not your last way
 * in; `kept` when you sign in with it otherwise; `none` when it is not one
 * of your ways in. `undefined` until known.
 */
export type WayInFate = "removed" | "kept" | "none";

/** Why starting was refused before anyone left the page. */
export type ConsentRefusal = "step_up" | "unsupported" | "rate_limited" | "failed";

export interface MailboxConsent {
  /** The providers a mailbox can be connected from, in display order (undefined until known). */
  providers: readonly string[] | undefined;
  /** The providers could not be read. */
  providersFailed: boolean;
  /** Start it; on success the browser leaves for the provider. */
  start: (request: ConsentStart) => void;
  /** A start is on its way (or the page is leaving). */
  starting: boolean;
  /** Why the last start was refused, if it was. */
  refusal: ConsentRefusal | undefined;
  /** Sign in again (a fresh sign-in), coming back to the page. */
  signInAgain: () => void;
  /** What disconnecting the mailbox `address` at `provider` does to your way in. */
  wayInFate: (provider: string, address: string) => WayInFate | undefined;
}

/** The hook the app shell hands to `buildConnectorsRoutes`. */
export type UseMailboxConsent = (options: {
  workspaceId: string;
  /** The page to come back to. */
  returnTo: string;
}) => MailboxConsent;
