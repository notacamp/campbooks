import { loginUrl } from "~/lib/api";
import { readTabValue, writeTabValue } from "~/lib/storage/tab";

/** The ways in this client can show, as `RequestInfo.providers` names them. */
const PROVIDERS = ["google", "microsoft", "zoho"] as const;
export type Provider = (typeof PROVIDERS)[number];

/** Shortcut ids for the provider buttons, by position (`1`, `2`, `3`). */
export const CHOICE_SHORTCUTS = ["sign_in.choice1", "sign_in.choice2", "sign_in.choice3"] as const;

export interface WaysIn {
  /** Providers the server lists that this client knows, in the server's order (at most three). */
  providers: Provider[];
  /** Whether the server lists passkeys. */
  passkey: boolean;
}

const isProvider = (name: string): name is Provider =>
  (PROVIDERS as readonly string[]).includes(name);

/** The ways in to show for a waiting request; names this client doesn't know are ignored. */
export function waysIn(listed: readonly string[]): WaysIn {
  return {
    providers: listed.filter(isProvider).slice(0, CHOICE_SHORTCUTS.length),
    passkey: listed.includes("passkey"),
  };
}

/** Where a provider button goes: the start route for this request. */
export function startUrl(method: Provider, request: string): string {
  return `/auth/start/${method}?request=${encodeURIComponent(request)}`;
}

/**
 * A path to come back to after signing in: same-origin only, and never a
 * sign-in screen (that would loop). Anything else becomes `/`.
 */
export function safeReturnTo(path: string | null | undefined): string {
  if (
    path === null ||
    path === undefined ||
    !path.startsWith("/") ||
    path.startsWith("//") ||
    path.startsWith("/\\") ||
    path === "/sign-in" ||
    path.startsWith("/sign-in/") ||
    path.startsWith("/sign-in?")
  ) {
    return "/";
  }
  return path;
}

/** Remembers, for this tab, where signing in should come back to. */
export function rememberReturnTo(path: string): void {
  writeTabValue("signIn.returnTo", safeReturnTo(path));
}

/** Where this tab's sign-in comes back to (`/` when nothing was remembered). */
export function rememberedReturnTo(): string {
  return safeReturnTo(readTabValue("signIn.returnTo"));
}

/** Starts a fresh sign-in through the BFF, coming back to the remembered page. */
export function startAgainUrl(): string {
  return loginUrl(rememberedReturnTo());
}

/** Whether the person is signing in to accept an invitation. */
export function isInvitation(returnTo: string): boolean {
  return returnTo.startsWith("/invitations/accept");
}

/** The callback errors the sign-in screen explains; anything else is `unknown`. */
const SIGN_IN_ERRORS = [
  "cancelled_at_provider",
  "invalid_state",
  "provider_unavailable",
  "login_failed",
  "account_disabled",
  "way_in_belongs_to_another_account",
  "signup_invite_only",
] as const;
export type SignInError = (typeof SIGN_IN_ERRORS)[number] | "unknown";

export function signInError(code: string): SignInError {
  return (SIGN_IN_ERRORS as readonly string[]).includes(code) ? (code as SignInError) : "unknown";
}

/** Navigates the whole page (to the BFF, a provider, the issuer). */
export function leavePage(url: string): void {
  globalThis.location.assign(url);
}

/** How the front door came back to the Ways to sign in page (`?linked=`, `?connected=`, `?link_error=`). */
export type LinkResult =
  | { kind: "linked"; provider: Provider }
  | { kind: "connected" }
  | { kind: "error"; error: LinkError };

/** The refusals the Ways to sign in page explains; anything else is `unknown`. */
const LINK_ERRORS = [
  "way_in_belongs_to_another_account",
  "sign_in_again",
  "link_expired",
  "cancelled_at_provider",
  "login_failed",
  "provider_unavailable",
  "consent_incomplete",
  "account_mismatch",
  "account_connected_by_another_member",
] as const;
type LinkError = (typeof LINK_ERRORS)[number] | "unknown";

function linkError(code: string): LinkError {
  return (LINK_ERRORS as readonly string[]).includes(code) ? (code as LinkError) : "unknown";
}

/** What the page's query says happened, if anything. */
export function linkResult(search: {
  linked?: string | undefined;
  connected?: string | undefined;
  link_error?: string | undefined;
}): LinkResult | undefined {
  if (search.link_error !== undefined) {
    return { kind: "error", error: linkError(search.link_error) };
  }
  if (search.linked !== undefined && isProvider(search.linked)) {
    return { kind: "linked", provider: search.linked };
  }
  return search.connected === undefined ? undefined : { kind: "connected" };
}

/**
 * Signs in again and comes back to `returnTo`: a sign-in at most ten minutes
 * old, or, `fresh`, a new one now (adding a passkey asks for one from the
 * last five minutes).
 */
export function stepUpUrl(returnTo: string, { fresh = false }: { fresh?: boolean } = {}): string {
  return fresh
    ? loginUrl(safeReturnTo(returnTo), { fresh: true })
    : `${loginUrl(safeReturnTo(returnTo))}&max_age=600`;
}

/** How long a new way to sign in stands out on Ways to sign in. */
export const RECENT_DAYS = 7;

/** Whether a way in added at `iso` is recent enough to stand out (as of `now`). */
export function addedRecently(iso: string, now: Date = new Date()): boolean {
  const age = now.getTime() - new Date(iso).getTime();
  return age >= 0 && age < RECENT_DAYS * 24 * 60 * 60 * 1000;
}

/** Whether `provider` is one this client can show. */
export function knownProvider(name: string): name is Provider {
  return isProvider(name);
}

/** What a workspace's own app may do, as Settings offers it: read, or read and change. */
export const APP_ACCESS = {
  read: ["system:read", "workspaces:read", "connectors:read", "conversations:read"],
  write: [
    "system:read",
    "workspaces:read",
    "connectors:read",
    "connectors:write",
    "conversations:read",
    "conversations:write",
    "conversations:send",
  ],
} as const satisfies Record<string, readonly string[]>;
export type AppAccess = keyof typeof APP_ACCESS;

/**
 * Whether `uri` may be a sign-in app's return address: `https` with a host,
 * or loopback `http` (`127.0.0.1`, `[::1]`, `localhost`); never a fragment
 * or credentials. The server checks again.
 */
export function isReturnAddress(uri: string): boolean {
  let url: URL;
  try {
    url = new URL(uri);
  } catch {
    return false;
  }
  if (url.hash !== "" || url.username !== "" || url.password !== "") {
    return false;
  }
  if (url.protocol === "https:") {
    return url.hostname !== "";
  }
  return url.protocol === "http:" && ["127.0.0.1", "[::1]", "localhost"].includes(url.hostname);
}
