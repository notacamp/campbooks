import {
  createContext,
  use,
  useLayoutEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import { writePref } from "~/lib/storage/prefs";

import type { Accent, Density, ThemeMode } from "../tokens/tokens";

import { applyTheme, DEFAULT_WORKSPACE, readStoredTheme, type ThemeSettings } from "./theme";

export interface ThemeContextValue extends ThemeSettings {
  /** What `system` resolves to right now. */
  resolvedTheme: "light" | "dark";
  setTheme: (theme: ThemeMode) => void;
  /** Sets the open workspace's accent (remembered per workspace). */
  setAccent: (accent: Accent) => void;
  setDensity: (density: Density) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

const DARK_QUERY = "(prefers-color-scheme: dark)";

/** The OS colour-scheme query, or null where matchMedia is missing (jsdom). */
function darkQuery(): MediaQueryList | null {
  return typeof globalThis.matchMedia === "function" ? globalThis.matchMedia(DARK_QUERY) : null;
}

function subscribeToScheme(onChange: () => void): () => void {
  const query = darkQuery();
  query?.addEventListener("change", onChange);
  return () => {
    query?.removeEventListener("change", onChange);
  };
}

const systemIsDark = () => darkQuery()?.matches ?? false;

export interface ThemeProviderProps {
  children: ReactNode;
  /** The open workspace; its accent is remembered separately. */
  workspaceId?: string | undefined;
  /** The element that carries the data attributes. Defaults to <html>. */
  target?: HTMLElement;
}

/**
 * Owns the theme (light/dark/system), the workspace accent and the density,
 * mirrors them onto <html> as data attributes and remembers them on this
 * device. `public/theme-boot.js` applies the same attributes before React
 * loads, so there is no flash on reload.
 */
export function ThemeProvider({
  children,
  workspaceId = DEFAULT_WORKSPACE,
  target,
}: ThemeProviderProps) {
  const [settings, setSettings] = useState(() => readStoredTheme(workspaceId));
  const [loadedFor, setLoadedFor] = useState(workspaceId);
  if (loadedFor !== workspaceId) {
    // Switching workspace swaps in that workspace's accent during render.
    setLoadedFor(workspaceId);
    setSettings(readStoredTheme(workspaceId));
  }

  useLayoutEffect(() => {
    applyTheme(settings, target ?? document.documentElement);
    writePref("workspace", workspaceId);
  }, [settings, target, workspaceId]);

  const prefersDark = useSyncExternalStore(subscribeToScheme, systemIsDark, () => false);

  const value = useMemo<ThemeContextValue>(
    () => ({
      ...settings,
      resolvedTheme:
        settings.theme === "system" ? (prefersDark ? "dark" : "light") : settings.theme,
      setTheme: (theme) => {
        writePref("theme", theme);
        setSettings((current) => ({ ...current, theme }));
      },
      setAccent: (accent) => {
        writePref(`accent.${workspaceId}`, accent);
        setSettings((current) => ({ ...current, accent }));
      },
      setDensity: (density) => {
        writePref("density", density);
        setSettings((current) => ({ ...current, density }));
      },
    }),
    [settings, prefersDark, workspaceId],
  );

  return <ThemeContext value={value}>{children}</ThemeContext>;
}

/** The current theme and its setters. Must be used under {@link ThemeProvider}. */
export function useTheme(): ThemeContextValue {
  const value = use(ThemeContext);
  if (value === null) {
    throw new Error("useTheme() must be used inside <ThemeProvider>");
  }
  return value;
}
