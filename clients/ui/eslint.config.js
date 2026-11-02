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

// @notacamp/ui's lint, the app's rules for the design system (AGENTS.md → UI).
// Token-only styling: Tailwind's default theme is wiped
// (src/tokens/tailwind-theme.css), so palette classes generate nothing;
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
// The package knows no app: no `~/` alias, nothing from a sibling app.
const APP_IMPORT_MESSAGE =
  "@notacamp/ui imports nothing from an app: it is shared by several. Move what you need into the package, or keep the component in the app.";

const WEB_STORAGE_MESSAGE =
  "Web Storage is readable by any script on the origin. Only the theme's default prefs adapter (src/theme/prefs.ts) touches it; an app passes its own.";

export default tseslint.config(
  {
    ignores: ["dist/"],
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
      // unicorn 79 rules switched off (the repo bans eslint-disable comments, so a
      // config-level decision):
      // - prefer-default-parameters also fires on `const [y, m, d] = s.split("-").map(Number)`
      //   with `?? fallback` (noUncheckedIndexedAccess) and turns `x ?? d` into a default that
      //   ignores null; it has no safe autofix here.
      // - prefer-await would rewrite ~50 deliberate fire-and-forget `.catch()`/`.then()`
      //   chains in event handlers and effects into async wrappers.
      "unicorn/prefer-default-parameters": "off",
      "unicorn/prefer-await": "off",
      // prefer-number-coercion rewrites `Number.parseFloat("16px") || 16` (computed styles,
      // CSS lengths) to `Number()`, which is NaN for any unit suffix.
      "unicorn/prefer-number-coercion": "off",
      // no-declarations-before-early-exit and prefer-minimal-ternary are style-only here:
      // the first flags grouped `const`s (module constants, test fixtures, closures) that
      // read better together; the second moves a ternary inside `t(...)`, which hides the
      // literal i18n keys from key extraction.
      "unicorn/no-declarations-before-early-exit": "off",
      "unicorn/prefer-minimal-ternary": "off",
      // prefer-math-min-max is not type-aware: it flags comparisons of `YYYY-MM-DD` strings,
      // where `Math.max` would return NaN.
      "unicorn/prefer-math-min-max": "off",
      // The TS lib target has neither `Promise.withResolvers` nor Iterator helpers (ES2023).
      "unicorn/prefer-promise-with-resolvers": "off",
      "unicorn/prefer-iterator-helpers": "off",
      "unicorn/filename-case": [
        "error",
        // kebab-case by default; PascalCase is for component files/folders.
        { cases: { kebabCase: true, pascalCase: true } },
      ],
      "@typescript-eslint/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
      "@typescript-eslint/no-import-type-side-effects": "error",
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: false }],
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            { group: ["~", "~/*"], message: APP_IMPORT_MESSAGE },
            { regex: String.raw`^(\.\./)+(app|clients)(/|$)`, message: APP_IMPORT_MESSAGE },
            { group: ["@campbooks/*"], message: APP_IMPORT_MESSAGE },
          ],
        },
      ],
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
    files: ["src/theme/prefs.ts"],
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
      // Untrusted HTML is rendered only through src/safe-html (sandboxed iframe).
      "react/no-danger": "error",
      "react/jsx-no-target-blank": "error",
      "react/no-unstable-nested-components": "error",
    },
  },
  {
    files: ["src/safe-html/**"],
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
    files: ["*.config.{js,ts}", "scripts/**/*.js"],
    languageOptions: { globals: globals.node },
  },
  {
    files: ["**/*.{js,cjs}"],
    ...tseslint.configs.disableTypeChecked,
  },
);
