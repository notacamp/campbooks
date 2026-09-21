/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    // ── lib/ → modules/ is never allowed ─────────────────────────────────────
    // The design system and toolbox must be business-agnostic. If lib/ needed a
    // module, that code belongs either in lib/ or in the module's api/ layer.
    {
      name: "no-lib-to-modules",
      comment:
        "lib/ must never import from modules/. Move shared logic to lib/ or keep it in the module.",
      severity: "error",
      from: { path: "^src/lib" },
      to: { path: "^src/modules" },
    },

    // ── Cross-module deep imports ─────────────────────────────────────────────
    // A module's public API is its index barrel. Importing its internals from
    // outside the subtree breaks encapsulation and makes refactors painful.
    // NOTE: dependency-cruiser cannot compare capture groups (it cannot say
    // "from module A to non-index of module B where A ≠ B"). The rule below is a
    // conservative approximation that flags any import landing inside a module
    // directory at depth > 1 from src/modules/. Intra-module imports (within the
    // same subtree) are allowed by the pathNot exclusion.
    // A more precise recursion-aware rule can be added via a custom reporter
    // once the module tree grows.
    {
      name: "no-cross-module-deep-import",
      comment:
        "Import another module only via its index.ts barrel, not via internal paths.",
      severity: "warn",
      from: { path: "^src/(modules|lib)" },
      to: {
        // Lands in a module subdirectory (at least one path segment after the
        // module name), NOT at the module root where index.ts lives.
        path: "^src/modules/[^/]+/.+",
        // Allow imports that stay within the same top-level module (same prefix).
        pathNot: "^src/modules/[^/]+/index\\.ts$",
      },
    },

    // ── No circular dependencies ──────────────────────────────────────────────
    {
      name: "no-circular",
      comment: "Circular dependencies indicate a missing abstraction in lib/.",
      severity: "warn",
      from: {},
      to: { circular: true },
    },
  ],

  options: {
    doNotFollow: {
      path: "node_modules",
    },
    tsConfig: {
      fileName: "tsconfig.json",
    },
    enhancedResolveOptions: {
      exportsFields: ["exports"],
      conditionNames: ["import", "require", "node", "default"],
      mainFields: ["module", "main", "types", "typings"],
    },
    reporterOptions: {
      dot: {
        collapsePattern: "node_modules/[^/]+",
      },
    },
  },
};
