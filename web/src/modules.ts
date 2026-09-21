/**
 * modules.ts — the module manifest: the composition root's knowledge of modules.
 *
 * Mounting a module here is the single switch for all of its contributions
 * (route builders, extension registrations, i18n slices, etc.). The closed
 * top-level list is enforced by scripts/module-policy.mjs.
 *
 * CLOSED LIST — the eight approved top-level modules:
 *   now       → the prioritised feed (email cards, ask cards, meetings)
 *   people    → contacts, organisations, inbox standings
 *   paper     → documents, attachments, AI fields
 *   money     → ledger, reconciliation, bank statements, loans
 *   time      → calendar, reminders, tasks, focus blocks
 *   scout     → AI chat, tool calls, memory, ⌘K
 *   settings  → account, workspace, integrations, AI config
 *   auth      → sign-in, registration, OTP, OAuth callbacks
 *
 * Each module exports:
 *   moduleConfig  — { extensions: AnyExtension[], i18nSlices?: string[] }
 *
 * Add a module:
 *   1. Create src/modules/<name>/ with the module contract (see architecture.md).
 *   2. Add its name to TOP_LEVEL_MODULES in scripts/module-policy.mjs.
 *   3. Import its moduleConfig here and add it to the modules array below.
 *   4. Wire its route builder in router.tsx.
 *
 * Submodules (sub-areas of a domain) nest at src/modules/<parent>/modules/<child>/
 * and need NONE of the above approval steps — they are internal to their parent.
 */
import { registerExtensions, registerGateResolver } from "~/lib/extensions";
import type { AnyExtension } from "~/lib/extensions";

// ── Module config type ────────────────────────────────────────────────────────

export interface ModuleConfig {
  /** Extensions this module contributes. */
  extensions?: AnyExtension[];
  /** Future: per-module i18n slice paths for lazy loading. */
  i18nSlices?: string[];
}

// ── Module imports (add here as modules are built) ────────────────────────────
import { moduleConfig as inbox } from "~/modules/inbox";
import { moduleConfig as settings } from "~/modules/settings";
//   import { moduleConfig as now }      from "~/modules/now";
//   import { moduleConfig as people }   from "~/modules/people";
//   import { moduleConfig as paper }    from "~/modules/paper";
//   import { moduleConfig as money }    from "~/modules/money";
//   import { moduleConfig as time }     from "~/modules/time";
//   import { moduleConfig as scout }    from "~/modules/scout";
//   import { moduleConfig as auth }     from "~/modules/auth";

// ── Composition root ──────────────────────────────────────────────────────────

const modules: ModuleConfig[] = [
  inbox,
  settings,
  // Module configs spread here as modules land:
  // now, people, paper, money, time, scout, auth,
];

/**
 * Register all module extensions and wire the feature-flag gate resolver.
 * Called once from main.tsx before the first render.
 *
 * The gate resolver is injected here (not in lib/extensions) so that
 * lib/extensions stays free of lib/feature-flags edges.
 */
export const mountModules = (): void => {
  // Gate resolver stub — replace with real feature-flag hook once a feature-flag
  // library is configured (GrowthBook, LaunchDarkly, etc.).
  registerGateResolver((_key: string): boolean | null => null);

  const extensions: AnyExtension[] = modules.flatMap(
    (m) => m.extensions ?? [],
  );
  registerExtensions(extensions);
};
