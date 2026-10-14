/**
 * Per-device UI preferences (language, theme, density, accent) in `localStorage`.
 *
 * This is the ONLY place allowed to touch Web Storage (enforced by ESLint).
 * NEVER store credentials, tokens, session ids or user data here: anything in
 * Web Storage is readable by any script on the origin (AGENTS.md, ADR 0004).
 * The one exception is the text a member typed into search: recent queries
 * may stay on the device, under a key scoped to the signed-in account and
 * workspace (`searches.<account>.<workspace>`) and cleared on sign-out
 * (`forgetPrefs("searches.")`). Never store results or anything from mail.
 * Keys are a closed union so adding one is a reviewed change.
 */
export type PrefKey =
  | "locale"
  | "theme"
  | "density"
  /** The last queries a member typed in search (a JSON array), per account and workspace. */
  | `searches.${string}`
  /** The workspace whose accent the boot script applies before first paint. */
  | "workspace"
  /** A workspace's accent preset: `accent.<workspace id>`. */
  | `accent.${string}`;

/** Also read by `public/theme-boot.js`, which runs before any module loads. */
export const PREF_PREFIX = "campbooks.pref.";

function storage(): Storage | null {
  try {
    return globalThis.localStorage;
  } catch {
    // Blocked storage (privacy mode, sandboxed iframe) behaves as empty.
    return null;
  }
}

export function readPref(key: PrefKey): string | null {
  try {
    return storage()?.getItem(PREF_PREFIX + key) ?? null;
  } catch {
    return null;
  }
}

export function writePref(key: PrefKey, value: string): void {
  try {
    storage()?.setItem(PREF_PREFIX + key, value);
  } catch {
    // Quota exceeded or storage disabled: preferences are best-effort.
  }
}

/** Removes every preference whose key starts with `prefix` (sign-out clears `searches.`). */
export function forgetPrefs(prefix: string): void {
  try {
    const store = storage();
    if (store === null) {
      return;
    }
    const start = PREF_PREFIX + prefix;
    const doomed: string[] = [];
    for (let index = 0; index < store.length; index += 1) {
      const key = store.key(index);
      if (key?.startsWith(start) === true) {
        doomed.push(key);
      }
    }
    for (const key of doomed) {
      store.removeItem(key);
    }
  } catch {
    // Storage blocked: there is nothing to forget.
  }
}
