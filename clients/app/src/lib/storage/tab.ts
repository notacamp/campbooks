/**
 * Values that live only as long as this browser tab, in `sessionStorage`.
 *
 * Same rules as `prefs.ts`: NEVER credentials, tokens, session ids or user
 * data. Keys are a closed union so adding one is a reviewed change.
 */
export type TabKey =
  /** The same-origin path to come back to once a sign-in finishes. */
  | "signIn.returnTo"
  /** The mailbox Add calendar access was started for (an id, to name it when the provider sends the member back). */
  | "calendarAccess.connection";

const TAB_PREFIX = "campbooks.tab.";

function storage(): Storage | null {
  try {
    return globalThis.sessionStorage;
  } catch {
    // Blocked storage (privacy mode, sandboxed iframe) behaves as empty.
    return null;
  }
}

export function readTabValue(key: TabKey): string | null {
  try {
    return storage()?.getItem(TAB_PREFIX + key) ?? null;
  } catch {
    return null;
  }
}

export function writeTabValue(key: TabKey, value: string): void {
  try {
    storage()?.setItem(TAB_PREFIX + key, value);
  } catch {
    // Storage disabled: the value is a convenience, not a requirement.
  }
}
