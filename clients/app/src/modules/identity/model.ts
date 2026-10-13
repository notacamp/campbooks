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

/** Where a provider (or a passkey) button goes: S1a's start route for this request. */
export function startUrl(method: Provider | "passkey", request: string): string {
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
] as const;
export type SignInError = (typeof SIGN_IN_ERRORS)[number] | "unknown";

export function signInError(code: string): SignInError {
  return (SIGN_IN_ERRORS as readonly string[]).includes(code) ? (code as SignInError) : "unknown";
}

/** Navigates the whole page (to the BFF, a provider, the issuer). */
export function leavePage(url: string): void {
  globalThis.location.assign(url);
}
