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

const WEB_STORAGE_MESSAGE =
  "Web Storage is readable by any script on the origin. Use ~/lib/storage for non-credential UI preferences; tokens never reach browser JS (ADR 0004).";

export default tseslint.config(
  {
    ignores: [
      "dist/",
      "coverage/",
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
    files: ["*.config.{js,ts}", "*.cjs", "test/**/*.ts", "scripts/**/*.ts", "recordings/**/*.ts"],
    languageOptions: { globals: globals.node },
  },
  {
    files: ["**/*.{js,cjs}"],
    ...tseslint.configs.disableTypeChecked,
  },
  {
    files: ["**/*.cjs"],
    languageOptions: { sourceType: "commonjs", globals: globals.node },
    rules: { "@typescript-eslint/no-require-imports": "off" },
  },
);
