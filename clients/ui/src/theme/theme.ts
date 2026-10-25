import {
  ACCENTS,
  DENSITIES,
  THEME_MODES,
  type Accent,
  type Density,
  type ThemeMode,
} from "../tokens/tokens";

import type { ThemePrefs } from "./prefs";

export interface ThemeSettings {
  theme: ThemeMode;
  accent: Accent;
  density: Density;
}

export const DEFAULT_THEME: ThemeSettings = {
  theme: "system",
  accent: "business",
  density: "comfortable",
};

/** The workspace id used until the app knows which workspace is open. */
export const DEFAULT_WORKSPACE = "default";

function oneOf<T extends string>(allowed: readonly T[], value: string | null, fallback: T): T {
  return value !== null && (allowed as readonly string[]).includes(value) ? (value as T) : fallback;
}

export const parseThemeMode = (value: string | null): ThemeMode =>
  oneOf(THEME_MODES, value, DEFAULT_THEME.theme);
export const parseAccent = (value: string | null): Accent =>
  oneOf(ACCENTS, value, DEFAULT_THEME.accent);
export const parseDensity = (value: string | null): Density =>
  oneOf(DENSITIES, value, DEFAULT_THEME.density);

/** The saved settings for a workspace on this device (defaults where unset). */
export function readStoredTheme(workspaceId: string, prefs: ThemePrefs): ThemeSettings {
  return {
    theme: parseThemeMode(prefs.read("theme")),
    accent: parseAccent(prefs.read(`accent.${workspaceId}`)),
    density: parseDensity(prefs.read("density")),
  };
}

/**
 * Writes the theme onto an element as data attributes; tokens.css does the
 * rest. Same attributes as `public/theme-boot.js`, which applies them before
 * first paint.
 */
export function applyTheme(settings: ThemeSettings, element: HTMLElement): void {
  element.dataset["theme"] = settings.theme;
  element.dataset["accent"] = settings.accent;
  element.dataset["density"] = settings.density;
}
