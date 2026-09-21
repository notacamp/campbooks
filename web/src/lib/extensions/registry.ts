/**
 * lib/extensions/registry — extension point mechanism.
 *
 * Pattern (from connect-web-v2 architecture):
 *   1. lib/extensions owns the mechanism only: Extension descriptor, registry,
 *      ExtensionPoint renderer. No concrete point names live here.
 *   2. A feature OWNER augments ExtensionPointContracts via declaration-merging
 *      in its own types.ts and renders <ExtensionPoint name="now.feed.sections" />.
 *   3. CONTRIBUTING modules declare their extensions in their moduleConfig
 *      barrel export: { id, point, order, load: () => import(...) }.
 *
 * Extensions are leaves (use their own module's api/), mount behind Suspense +
 * error boundary (a throwing extension blanks itself, never the page), and are
 * gated per-descriptor (not inside the component).
 */

// ── Types ─────────────────────────────────────────────────────────────────────

import type { ReactNode } from "react";

/**
 * Declaration-merged by each extension-point owner.
 * Maps point name → props interface.
 *
 * Example (in now/types.ts):
 *   declare module "~/lib/extensions" {
 *     interface ExtensionPointContracts {
 *       "now.feed.sections": { date: Date };
 *     }
 *   }
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export interface ExtensionPointContracts {}

export type ExtensionPointName = keyof ExtensionPointContracts;

/** An extension that contributes to a named point. */
export interface Extension<N extends ExtensionPointName> {
  id: string;
  point: N;
  order: number;
  /** Optional feature-flag key; a falsy resolved value disables this extension. */
  flag?: string;
  load: () => Promise<{ default: (props: ExtensionPointContracts[N]) => ReactNode }>;
}

/** Use this as the type for moduleConfig extensions (avoids bare Extension<N>). */
export type AnyExtension = Extension<ExtensionPointName>;

// ── Registry ──────────────────────────────────────────────────────────────────

const registry = new Map<ExtensionPointName, AnyExtension[]>();

/** Register one or more extensions. Called from app/modules.ts via mountModules. */
export const registerExtensions = (extensions: AnyExtension[]): void => {
  for (const ext of extensions) {
    const list = registry.get(ext.point) ?? [];
    list.push(ext);
    list.sort((a, b) => a.order - b.order);
    registry.set(ext.point, list);
  }
};

/** Returns all extensions registered for a given point, sorted by order. */
export const getExtensions = (point: ExtensionPointName): AnyExtension[] =>
  registry.get(point) ?? [];

// ── Gate resolver injection ───────────────────────────────────────────────────
// lib/extensions never imports lib/feature-flags. The gate resolver is injected
// from app/modules.ts (the only file allowed to bridge the two).

type GateResolver = (flag: string) => boolean | null;

let gateResolver: GateResolver | null = null;

/** Injected by app/modules.ts so lib/extensions stays free of feature-flag edges. */
export const registerGateResolver = (resolver: GateResolver): void => {
  gateResolver = resolver;
};

/** True when the extension's flag is enabled (or when no flag is set). */
export const isExtensionEnabled = (ext: AnyExtension): boolean => {
  if (!ext.flag) return true;
  if (!gateResolver) return false; // gates not yet wired → safe close
  const resolved = gateResolver(ext.flag);
  return resolved === true;
};
