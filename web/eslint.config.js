import js from "@eslint/js";
import globals from "globals";
import typescript from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import jsxA11y from "eslint-plugin-jsx-a11y";
import unicorn from "eslint-plugin-unicorn";
import testingLibrary from "eslint-plugin-testing-library";
import vitestPlugin from "@vitest/eslint-plugin";

import typedArrowFunctions from "./eslint-rules/typed-arrow-functions.mjs";
import namedPackageImports from "./eslint-rules/named-package-imports.mjs";
import noRawHex from "./eslint-rules/no-raw-hex.mjs";
import noRawOklch from "./eslint-rules/no-raw-oklch.mjs";
import noTwRadius from "./eslint-rules/no-tw-radius.mjs";

// Local plugin wrapping the custom rules so they are referenced as
// `local/<name>`.
const local = {
  rules: {
    "typed-arrow-functions": typedArrowFunctions,
    "named-package-imports": namedPackageImports,
    "no-raw-hex": noRawHex,
    "no-raw-oklch": noRawOklch,
    "no-tw-radius": noTwRadius,
  },
};

export default [
  // ── Global ignores ──────────────────────────────────────────────────────────
  {
    ignores: [
      "node_modules/**",
      "dist/**",
      ".dependency-cruiser.cjs",
    ],
  },

  // ── Base JS recommended ─────────────────────────────────────────────────────
  js.configs.recommended,

  // ── TypeScript app source (src/) ────────────────────────────────────────────
  {
    files: ["src/**/*.ts", "src/**/*.tsx"],
    plugins: {
      "@typescript-eslint": typescript,
      react,
      "react-hooks": reactHooks,
      "jsx-a11y": jsxA11y,
      unicorn,
      local,
    },
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
      globals: {
        ...globals.browser,
        ...globals.es2022,
      },
    },
    settings: {
      react: { version: "detect" },
    },
    rules: {
      // ── Custom rules ──────────────────────────────────────────────────────
      "local/typed-arrow-functions": "error",
      "local/named-package-imports": [
        "error",
        { denyDefault: ["react", "react-dom"] },
      ],
      // Design-system guards (b4 spec):
      "local/no-raw-hex": "error",    // no #rrggbb literals — use CSS tokens
      "local/no-raw-oklch": "error",  // no oklch(0.x ...) literals — use tokens
      "local/no-tw-radius": "warn",   // prefer rounded-cb-* over rounded-sm/md/lg/xl

      // ── TypeScript ────────────────────────────────────────────────────────
      ...typescript.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/consistent-type-imports": [
        "error",
        { prefer: "type-imports", fixStyle: "inline-type-imports" },
      ],

      // ── React ─────────────────────────────────────────────────────────────
      ...react.configs.recommended.rules,
      "react/react-in-jsx-scope": "off",
      "react/prop-types": "off",
      "react/display-name": "warn",

      // ── React hooks ───────────────────────────────────────────────────────
      ...reactHooks.configs.recommended.rules,

      // ── Accessibility ─────────────────────────────────────────────────────
      ...jsxA11y.configs.recommended.rules,

      // ── Unicorn ───────────────────────────────────────────────────────────
      "unicorn/prevent-abbreviations": "off",
      "unicorn/filename-case": "off",
      "unicorn/no-null": "off",
      "unicorn/no-array-reduce": "off",

      // ── General quality ───────────────────────────────────────────────────
      // Turn off no-undef for TypeScript files — TS's own type checker handles
      // undefined references more precisely than ESLint's runtime globals list.
      // (Browser DOM types like RequestInit and Fetch are TS lib types, not in
      //  the ESLint globals package.)
      "no-undef": "off",
      "no-console": "error",
      "no-debugger": "error",
      // Allow _-prefixed parameters that are intentionally unused.
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],

      // State/data library bans — Zustand + TanStack Query are the only ones.
      "no-restricted-imports": [
        "error",
        {
          paths: [
            { name: "redux", message: "Client state uses Zustand only." },
            { name: "react-redux", message: "Client state uses Zustand only." },
            { name: "@reduxjs/toolkit", message: "Client state uses Zustand only." },
            { name: "jotai", message: "Client state uses Zustand only." },
            { name: "recoil", message: "Client state uses Zustand only." },
            { name: "valtio", message: "Client state uses Zustand only." },
            { name: "mobx", message: "Client state uses Zustand only." },
            { name: "swr", message: "Server state uses TanStack Query, not SWR." },
            {
              name: "@apollo/client",
              message: "Server state uses TanStack Query over REST.",
            },
            {
              name: "react-query",
              message: "Use @tanstack/react-query (v5).",
            },
            // createRouter / createRootRoute are pinned to src/router.tsx only.
            // Configured as a warning here; the router.tsx itself overrides it.
          ],
        },
      ],
    },
  },

  // ── Module internals: no HTML elements or className/style ────────────────────
  // Apply only to src/modules/ — lib/ui owns the design layer.
  {
    files: ["src/modules/**/*.{ts,tsx}"],
    plugins: { react, local },
    languageOptions: {
      parser: tsParser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    rules: {
      // No native HTML outside lib/ui; modules must use lib/ui components.
      "react/forbid-elements": [
        "error",
        {
          forbid: [
            "div", "span", "p", "section", "article", "header", "footer",
            "main", "nav", "aside", "ul", "ol", "li", "button", "input",
            "label", "form", "select", "textarea", "h1", "h2", "h3",
            "h4", "h5", "h6", "a", "img",
          ],
        },
      ],
      // No className or style props on components — directive props only.
      "react/forbid-component-props": [
        "error",
        { forbid: ["className", "style"] },
      ],
      // i18n required in modules — no hardcoded JSX strings.
      "react/jsx-no-literals": ["warn", { noStrings: true }],
    },
  },

  // ── Surface views (presentation layer inside modules) ───────────────────────
  // modules/<surface>/components/** are the presentation/layout layer (like
  // lib/ui), so raw HTML here is a TRACKED, TEMPORARY relaxation — warn, not
  // error. connect-web-v2 end-state = compose lib/ui layout primitives
  // (Box/Stack/Grid) instead of div/span; that's a fast-follow. jsx-no-literals
  // stays warn too (react-intl extraction is a separate follow-up).
  {
    files: ["src/modules/**/components/**/*.{ts,tsx}"],
    plugins: { react },
    languageOptions: {
      parser: tsParser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    rules: {
      "react/forbid-elements": "warn",
    },
  },

  // ── Logger: allow console.* ──────────────────────────────────────────────────
  {
    files: ["src/lib/logger/**/*.{ts,tsx}"],
    rules: {
      "no-console": "off",
    },
  },

  // ── Tests ─────────────────────────────────────────────────────────────────────
  {
    files: [
      "src/**/*.test.{ts,tsx}",
      "src/**/*.spec.{ts,tsx}",
      "src/test/**/*.{ts,tsx}",
    ],
    plugins: {
      "testing-library": testingLibrary,
      vitest: vitestPlugin,
    },
    languageOptions: {
      parser: tsParser,
      globals: {
        ...globals.browser,
        ...vitestPlugin.environments?.env?.globals,
      },
    },
    rules: {
      ...testingLibrary.configs.react.rules,
      ...vitestPlugin.configs.recommended.rules,
      // Only data-testid selectors are allowed — no role/text/label lookups.
      "testing-library/consistent-data-testid": [
        "error",
        {
          testIdPattern:
            "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9-]*)+$",
          testIdAttribute: ["data-testid"],
        },
      ],
      // relax jsx-no-literals in tests
      "react/jsx-no-literals": "off",
      // Allow console in tests for debugging
      "no-console": "off",
    },
  },

  // ── Scripts (plain .mjs) ─────────────────────────────────────────────────────
  {
    files: ["scripts/**/*.mjs", "eslint-rules/**/*.mjs"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { ...globals.node },
    },
    rules: {
      "no-console": "off",
    },
  },
];
