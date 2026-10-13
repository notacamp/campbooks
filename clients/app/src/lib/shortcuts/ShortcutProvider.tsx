import {
  createContext,
  use,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import { DEFAULT_SHORTCUTS, type ShortcutId } from "./catalog";
import { createDispatcher } from "./dispatcher";
import { detectPlatform, type Platform } from "./keys";
import { ShortcutRegistry, type ShortcutDefinition, type ShortcutHandler } from "./registry";

interface ShortcutContextValue {
  registry: ShortcutRegistry;
  platform: Platform;
}

let fallback: ShortcutContextValue | undefined;

/** Used outside a provider (stories, tests): the defaults, no key listener. */
function fallbackContext(): ShortcutContextValue {
  fallback ??= {
    registry: new ShortcutRegistry(DEFAULT_SHORTCUTS),
    platform: typeof navigator === "undefined" ? "other" : detectPlatform(),
  };
  return fallback;
}

const ShortcutContext = createContext<ShortcutContextValue | null>(null);

export interface ShortcutProviderProps {
  children: ReactNode;
  /** Defaults to a registry holding {@link DEFAULT_SHORTCUTS}. */
  registry?: ShortcutRegistry;
  /** Defaults to the detected platform (⌘ on Apple, Ctrl elsewhere). */
  platform?: Platform;
}

/** Installs the single keydown listener and shares the registry. */
export function ShortcutProvider({ children, registry, platform }: ShortcutProviderProps) {
  const value = useMemo<ShortcutContextValue>(
    () => ({
      registry: registry ?? new ShortcutRegistry(DEFAULT_SHORTCUTS),
      platform: platform ?? detectPlatform(),
    }),
    [registry, platform],
  );

  useEffect(() => {
    const onKeyDown = createDispatcher(value.registry, value.platform);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [value]);

  return <ShortcutContext value={value}>{children}</ShortcutContext>;
}

/** The registry and platform in effect (the defaults outside a provider). */
export function useShortcuts(): ShortcutContextValue {
  return use(ShortcutContext) ?? fallbackContext();
}

/** One shortcut's definition, or undefined when no such id is defined. */
export function useShortcutDefinition(id: ShortcutId): ShortcutDefinition | undefined {
  const { registry } = useShortcuts();
  useSyncExternalStore(registry.subscribe, registry.getVersion, registry.getVersion);
  return registry.get(id);
}

/** Every definition, re-rendering when definitions or bindings change. */
export function useShortcutDefinitions(): {
  definitions: ShortcutDefinition[];
  isBound: (id: string) => boolean;
} {
  const { registry } = useShortcuts();
  useSyncExternalStore(registry.subscribe, registry.getVersion, registry.getVersion);
  return { definitions: registry.definitions(), isBound: (id) => registry.isBound(id) };
}

export interface UseShortcutOptions {
  /** Bind only while true (e.g. only for the selected row). Default true. */
  enabled?: boolean;
}

/**
 * Binds `handler` to the shortcut `id` while the component is mounted (and
 * `enabled`). The latest handler is always called; re-renders don't rebind.
 */
export function useShortcut(
  id: ShortcutId,
  handler: ShortcutHandler,
  { enabled = true }: UseShortcutOptions = {},
): void {
  const { registry } = useShortcuts();
  const latest = useRef(handler);
  useEffect(() => {
    latest.current = handler;
  });
  useEffect(() => {
    if (!enabled) {
      return;
    }
    return registry.bind(id, (event) => {
      latest.current(event);
    });
  }, [registry, id, enabled]);
}
