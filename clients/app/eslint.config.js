// @ts-check
import eslintComments from "@eslint-community/eslint-plugin-eslint-comments";
import js from "@eslint/js";
import vitest from "@vitest/eslint-plugin";
import jsxA11y from "eslint-plugin-jsx-a11y";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import unicorn from "eslint-plugin-unicorn";
import globals from "globals";
import tseslint from "typescript-eslint";

// Token-only styling (AGENTS.md → UI). Tailwind's default theme is wiped
// (src/lib/ui/tokens/tailwind-theme.css), so palette classes generate nothing;
// these selectors also reject arbitrary values (`p-[13px]`, `text-[#123]`),
// palette names and static duration/z-index utilities wherever class names are
// written: className, and calls to cx().
const CLASS_CONTEXTS = ['JSXAttribute[name.name="className"]', 'CallExpression[callee.name="cx"]'];
const classLiteralRule = (pattern, message) =>
  CLASS_CONTEXTS.flatMap((context) => [
    { selector: `${context} Literal[value=${pattern}]`, message },
    { selector: `${context} TemplateElement[value.raw=${pattern}]`, message },
  ]);
const TOKEN_CLASS_RULES = [
  ...classLiteralRule(
    String.raw`/(^|\s)[\w:!-]*\[[^\]]*\]/`,
    "Tailwind arbitrary values bypass the design tokens. Use a token utility (p-3, bg-surface-raised) or a lib/ui component.",
  ),
  ...classLiteralRule(
    String.raw`/(^|\s)([\w-]+:)*!?(bg|text|border|ring|outline|fill|stroke|from|via|to|decoration|shadow|accent|caret|divide|placeholder)-(slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|black|white)(\b|-)/`,
    "Palette colours are not tokens. Use a semantic utility (bg-surface-raised, text-secondary, border-subtle).",
  ),
  ...classLiteralRule(
    String.raw`/(^|\s)([\w-]+:)*(duration|delay|z)-\d/`,
    "Durations and z-index come from tokens (--duration-*, --z-*), inside lib/ui components.",
  ),
];
// In modules/** classes are for layout only: everything visual comes from a
// lib/ui component, so a module can't drift from the design system.
const MODULE_LAYOUT_ONLY_RULES = classLiteralRule(
  String.raw`/(^|\s)([\w-]+:)*!?(bg-|text-(?!(left|center|right|start|end|wrap|nowrap|balance|pretty)\b)|font-|rounded|shadow|ring|outline-|border|opacity-|tracking-|leading-|fill-|stroke-|decoration-|underline|italic|uppercase|lowercase|capitalize)/`,
  "modules/ use Tailwind for layout only (flex, grid, gap, padding, width). Colour, type, borders and radius come from lib/ui components.",
);
const NO_STYLE_PROP =
  "Inline styles bypass the design tokens. Use a lib/ui component (or layout utilities).";

const WEB_STORAGE_MESSAGE =
  "Web Storage is readable by any script on the origin. Use ~/lib/storage for non-credential UI preferences; tokens never reach browser JS (ADR 0004).";

export default tseslint.config(
  {
    ignores: [
      "dist/",
      "coverage/",
      "storybook-static/",
      "src-tauri/",
      "recordings/out/",
      // Generated from openapi/openapi.json (`pnpm api:generate`); never hand-edited.
      "src/lib/api/schema.gen.ts",
      // Intentionally boundary-violating sources for the dependency-cruiser test.
      "test/fixtures/",
    ],
  },
  {
    linterOptions: {
      // Inline `eslint-disable` comments are not honoured at all, and any that
      // exist are reported (AGENTS.md: no eslint-disable). Fix the code or
      // change the rule here, in review.
      noInlineConfig: true,
      reportUnusedDisableDirectives: "error",
    },
  },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: { allowDefaultProject: ["*.js", "*.cjs"] },
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  unicorn.configs.unopinionated,
  {
    plugins: { "@eslint-community/eslint-comments": eslintComments },
    rules: {
      "@eslint-community/eslint-comments/no-use": ["error", { allow: [] }],
      "unicorn/filename-case": [
        "error",
        // kebab-case by default; PascalCase is for component files/folders.
        { cases: { kebabCase: true, pascalCase: true } },
      ],
      "@typescript-eslint/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
      "@typescript-eslint/no-import-type-side-effects": "error",
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: false }],
      "no-restricted-globals": [
        "error",
        { name: "localStorage", message: WEB_STORAGE_MESSAGE },
        { name: "sessionStorage", message: WEB_STORAGE_MESSAGE },
      ],
      "no-restricted-properties": [
        "error",
        ...["window", "globalThis", "self"].flatMap((object) => [
          { object, property: "localStorage", message: WEB_STORAGE_MESSAGE },
          { object, property: "sessionStorage", message: WEB_STORAGE_MESSAGE },
        ]),
        {
          object: "document",
          property: "cookie",
          message:
            "Session cookies are httpOnly and owned by the BFF; JS must not read or write cookies.",
        },
      ],
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    rules: { "no-restricted-syntax": ["error", ...TOKEN_CLASS_RULES] },
  },
  {
    files: ["src/modules/**/*.{ts,tsx}", "src/app/**/*.{ts,tsx}"],
    rules: {
      "react/forbid-dom-props": [
        "error",
        { forbid: [{ propName: "style", message: NO_STYLE_PROP }] },
      ],
      "react/forbid-component-props": [
        "error",
        { forbid: [{ propName: "style", message: NO_STYLE_PROP }] },
      ],
    },
  },
  {
    files: ["src/modules/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-syntax": ["error", ...TOKEN_CLASS_RULES, ...MODULE_LAYOUT_ONLY_RULES],
    },
  },
  {
    files: ["src/lib/storage/**"],
    rules: { "no-restricted-globals": "off", "no-restricted-properties": "off" },
  },
  {
    files: ["**/*.{ts,tsx}"],
    ...react.configs.flat.recommended,
    ...react.configs.flat["jsx-runtime"],
    languageOptions: {
      ...react.configs.flat.recommended?.languageOptions,
      globals: globals.browser,
    },
    settings: { react: { version: "detect" } },
  },
  {
    files: ["**/*.{ts,tsx}"],
    rules: {
      ...react.configs.flat.recommended?.rules,
      ...react.configs.flat["jsx-runtime"]?.rules,
      // Untrusted HTML is rendered only through lib/ui/safe-html (sandboxed iframe).
      "react/no-danger": "error",
      "react/jsx-no-target-blank": "error",
      "react/no-unstable-nested-components": "error",
    },
  },
  {
    files: ["src/lib/ui/safe-html/**"],
    rules: { "react/no-danger": "off" },
  },
  reactHooks.configs.flat["recommended-latest"],
  jsxA11y.flatConfigs.strict,
  {
    files: ["**/*.test.{ts,tsx}"],
    ...vitest.configs.recommended,
    rules: {
      ...vitest.configs.recommended.rules,
      "vitest/consistent-test-it": ["error", { fn: "it" }],
      "vitest/no-focused-tests": "error",
      "vitest/no-disabled-tests": "error",
      "vitest/require-top-level-describe": "error",
    },
  },
  {
    files: [
      "*.config.{js,ts}",
      "*.cjs",
      "test/**/*.ts",
      "scripts/**/*.ts",
      "recordings/**/*.ts",
      ".storybook/main.ts",
    ],
    languageOptions: { globals: globals.node },
  },
  {
    files: ["**/*.{js,cjs}"],
    ...tseslint.configs.disableTypeChecked,
  },
  {
    // Runs before any module (see the file): it must read the saved theme
    // straight from Web Storage, with the same key prefix as lib/storage.
    files: ["public/theme-boot.js"],
    languageOptions: { sourceType: "script", globals: globals.browser },
    rules: { "no-restricted-globals": "off", "no-restricted-properties": "off" },
  },
  {
    files: ["**/*.cjs"],
    languageOptions: { sourceType: "commonjs", globals: globals.node },
    rules: { "@typescript-eslint/no-require-imports": "off" },
  },
);
