/**
 * Per-device UI preferences (language, theme, density, accent) in `localStorage`.
 *
 * This is the ONLY place allowed to touch Web Storage (enforced by ESLint).
 * NEVER store credentials, tokens, session ids or user data here: anything in
 * Web Storage is readable by any script on the origin (AGENTS.md, ADR 0004).
 * Keys are a closed union so adding one is a reviewed change.
 */
export type PrefKey =
  | "locale"
  | "theme"
  | "density"
  /** The last queries typed in search (a JSON array of strings), newest first. */
  | "searches"
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
