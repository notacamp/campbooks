import {
  ShortcutProvider as DesignSystemShortcutProvider,
  ShortcutRegistry,
  type ShortcutProviderProps,
} from "@notacamp/ui/shortcuts";
import { useMemo } from "react";

import { DEFAULT_SHORTCUTS } from "./catalog";

/**
 * The design system's provider, defaulting to a registry of Campbooks'
 * {@link DEFAULT_SHORTCUTS} (the app passes its own, with every module's).
 */
export function ShortcutProvider({ registry, ...props }: ShortcutProviderProps) {
  const defaults = useMemo(() => registry ?? new ShortcutRegistry(DEFAULT_SHORTCUTS), [registry]);
  return <DesignSystemShortcutProvider registry={defaults} {...props} />;
}
