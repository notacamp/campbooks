/** What the theme remembers: the device's theme and density, the last open workspace and each workspace's accent. */
export type ThemePrefKey = "theme" | "density" | "workspace" | `accent.${string}`;

/**
 * Where the theme remembers its settings on this device. An app passes its
 * own (Campbooks: its `lib/storage` prefs) so one store, one key prefix and
 * one boot script cover every preference.
 */
export interface ThemePrefs {
  /** The saved value, or null when there is none (or storage is blocked). */
  read: (key: ThemePrefKey) => string | null;
  /** Best effort: a full or blocked storage drops the write. */
  write: (key: ThemePrefKey, value: string) => void;
}

function storage(): Storage | null {
  try {
    return globalThis.localStorage;
  } catch {
    // Blocked storage (privacy mode, sandboxed iframe) behaves as empty.
    return null;
  }
}

/**
 * Preferences in `localStorage` under `prefix`. Never store credentials or
 * user data in Web Storage: any script on the origin can read it.
 */
export function localStoragePrefs(prefix = "ui.pref."): ThemePrefs {
  return {
    read: (key) => {
      try {
        return storage()?.getItem(prefix + key) ?? null;
      } catch {
        return null;
      }
    },
    write: (key, value) => {
      try {
        storage()?.setItem(prefix + key, value);
      } catch {
        // Quota exceeded or storage disabled: preferences are best-effort.
      }
    },
  };
}
