import {
  ThemeProvider as DesignSystemThemeProvider,
  type ThemePrefs,
  type ThemeProviderProps,
} from "@notacamp/ui";

import { readPref, writePref } from "~/lib/storage/prefs";

/**
 * The theme remembered with the app's other preferences, under the prefix
 * `public/theme-boot.js` reads before first paint.
 */
export const APP_THEME_PREFS: ThemePrefs = { read: readPref, write: writePref };

/** The design system's ThemeProvider, remembering through {@link APP_THEME_PREFS}. */
export function ThemeProvider(props: ThemeProviderProps) {
  return <DesignSystemThemeProvider prefs={APP_THEME_PREFS} {...props} />;
}
