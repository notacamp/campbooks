#!/usr/bin/env node
/**
 * module-policy.mjs — Campbooks top-level module registry and policy gate.
 *
 * The closed list of approved top-level modules lives here. Every directory
 * under `src/modules/` must be on this list, and every name on the list must
 * have its directory — both directions are checked.
 *
 * Adding a top-level module is a product decision, not a code detail. It
 * requires updating this list (with the surface it serves and what it holds)
 * in the same PR that creates the directory.
 *
 * Nearly every new area is a SUBMODULE nested at
 * `src/modules/<parent>/modules/<child>` — cheaper, unguarded, and correct for
 * any feature that belongs to an existing domain.
 *
 * Usage:
 *   node scripts/module-policy.mjs check   # exits 1 on any failure
 */

import { readdirSync } from "node:fs";
import { join } from "node:path";

/**
 * Every approved top-level module.
 * `surface` is the app place it drives; `holds` is what it owns.
 */
export const TOP_LEVEL_MODULES = [
  {
    name: "inbox",
    surface: "Inbox",
    holds: "the unified mail inbox: rows, tabs, archive/undo, reading pane, and realtime sync",
  },
  {
    name: "now",
    surface: "Now",
    holds: "the prioritised feed: email cards, ask cards, meeting cards, and the Scout strip",
  },
  {
    name: "people",
    surface: "People",
    holds: "contacts, organisations, sender threads, and the inbox standing lanes",
  },
  {
    name: "paper",
    surface: "Paper",
    holds: "documents, attachments, AI-analysed fields, and document types",
  },
  {
    name: "money",
    surface: "Money",
    holds: "the ledger, reconciliation, bank statements, loans, and the evidence model",
  },
  {
    name: "time",
    surface: "Time",
    holds: "calendar events, reminders, asks (tasks), focus blocks, and the agenda",
  },
  {
    name: "scout",
    surface: "Scout overlay",
    holds: "the AI chat, tool calls, memory, email draft, and the ⌘K command menu",
  },
  {
    name: "settings",
    surface: "Settings",
    holds: "account, workspace, integrations, AI, and inbox configuration",
  },
  {
    name: "auth",
    surface: "Auth pages",
    holds: "sign-in, registration, OTP, password reset, and OAuth callbacks",
  },
];

/** The approved names as a plain array — for membership tests. */
export const APPROVED_MODULE_NAMES = TOP_LEVEL_MODULES.map((m) => m.name);

/**
 * Returns the directory entries that actually exist under `src/modules/`.
 * Falls back gracefully if the directory does not exist yet.
 */
export const actualModuleDirs = (root = process.cwd()) => {
  const modulesPath = join(root, "src", "modules");
  try {
    return readdirSync(modulesPath, { withFileTypes: true })
      .filter(
        (entry) => entry.isDirectory() && !entry.name.startsWith("."),
      )
      .map((entry) => entry.name)
      .sort();
  } catch {
    return [];
  }
};

const unapproved = (name) =>
  `src/modules/${name} is not an approved top-level module.\n` +
  `  A top-level module is a product decision. To add one:\n` +
  `  1. Add an entry to TOP_LEVEL_MODULES in scripts/module-policy.mjs.\n` +
  `  2. Include the surface it drives and what it holds.\n` +
  `  3. Do this in the same PR that creates the directory.\n` +
  `  If this is a sub-area of an existing domain, use a submodule instead:\n` +
  `  src/modules/<parent>/modules/${name}`;

const missing = (name) =>
  `src/modules/${name} is listed in TOP_LEVEL_MODULES but the directory is missing.\n` +
  `  Removing or renaming a module requires updating TOP_LEVEL_MODULES in\n` +
  `  scripts/module-policy.mjs in the same PR.`;

/**
 * Returns all policy failures as an array of human-readable messages.
 * Empty when the list and the filesystem agree exactly.
 */
export const policyFailures = (
  root = process.cwd(),
  approved = APPROVED_MODULE_NAMES,
) => {
  const actual = actualModuleDirs(root);
  return [
    ...actual
      .filter((name) => !approved.includes(name))
      .map(unapproved),
    ...approved
      .filter((name) => !actual.includes(name))
      .map(missing),
  ];
};

if (process.argv[2] === "check") {
  const failures = policyFailures();
  for (const failure of failures) {
    console.error(failure);
  }
  process.exit(failures.length > 0 ? 1 : 0);
}
