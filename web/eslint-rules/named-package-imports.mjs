/**
 * named-package-imports — Campbooks re-implementation.
 *
 * Packages are imported by the names used, not as a single namespace object.
 * Two reports, both on npm package specifiers only (bare, no `.` or `/` prefix):
 *
 *   import * as React from "react"
 *     → `import { useState, … } from "react"`  (namespace ban)
 *
 *   import React from "react"  (when "react" is in denyDefault)
 *     → React's "default" is the whole module; import the names instead.
 *
 * `import type * as …` is erased at compile time → not reported.
 * Path imports (`./foo`, `~/lib/x`) are never reported — tests legitimately
 * take a namespace import of the module they spy on.
 *
 * Options (optional object):
 *   allowNamespace: string[]  — package prefixes whose namespace form is ok
 *   denyDefault:   string[]  — package names whose default import is the whole
 *                              module object (not a real default export)
 */

/** True for bare npm package specifiers; false for paths and scoped paths. */
const isPackage = (source) => !/^[./~]/.test(source);

export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Import a package by the names used, never as a namespace or a whole-module default.",
    },
    schema: [
      {
        type: "object",
        properties: {
          allowNamespace: { type: "array", items: { type: "string" } },
          denyDefault: { type: "array", items: { type: "string" } },
        },
        additionalProperties: false,
      },
    ],
    messages: {
      namespace:
        'Use named imports from "{{source}}" — `import { x } from "{{source}}"` — not `{{local}}` (the whole package).',
      default:
        '"{{source}}" has no standalone default export; `{{local}}` is the whole package. Use named imports instead.',
    },
  },

  create(context) {
    const options = context.options[0] ?? {};
    const allowNamespace = options.allowNamespace ?? [];
    const denyDefault = options.denyDefault ?? [];

    return {
      ImportDeclaration(node) {
        const source = node.source.value;
        if (typeof source !== "string" || !isPackage(source)) return;
        // Type-only imports are erased at runtime — not a concern.
        if (node.importKind === "type") return;

        const namespaceAllowed = allowNamespace.some((prefix) =>
          source.startsWith(prefix),
        );
        const defaultDenied = denyDefault.includes(source);

        for (const specifier of node.specifiers) {
          if (
            specifier.type === "ImportNamespaceSpecifier" &&
            !namespaceAllowed
          ) {
            context.report({
              node: specifier,
              messageId: "namespace",
              data: { source, local: specifier.local.name },
            });
          }
          if (specifier.type === "ImportDefaultSpecifier" && defaultDenied) {
            context.report({
              node: specifier,
              messageId: "default",
              data: { source, local: specifier.local.name },
            });
          }
        }
      },
    };
  },
};
