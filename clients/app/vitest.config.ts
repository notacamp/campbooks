import { fileURLToPath, URL } from "node:url";

import { defineConfig, mergeConfig } from "vitest/config";

import viteConfig from "./vite.config.ts";

// The design system's source (clients/ui/src). Coverage matches a file outside
// this package's root by its absolute path, so its pattern is absolute too.
const designSystem = fileURLToPath(new URL("../ui/src", import.meta.url)).replaceAll("\\", "/");

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      setupFiles: ["./test/setup.ts"],
      include: [
        "src/**/*.test.{ts,tsx}",
        "test/**/*.test.{ts,tsx}",
        "scripts/**/*.test.ts",
        // The design system's tests run here, against the app's toolchain, and
        // count toward the same coverage gate (clients/ui has no runner).
        "../ui/src/**/*.test.{ts,tsx}",
      ],
      exclude: ["test/fixtures/**"],
      restoreMocks: true,
      // Above Testing Library's async timeout (test/setup.ts): screens load
      // their i18n namespaces lazily on first render.
      testTimeout: 15_000,
      // Process CSS so component class names are real and token files can be
      // read with `?raw` (tokens.test.ts).
      css: true,
      unstubGlobals: true,
      coverage: {
        provider: "v8",
        // `../ui/src/**` finds its files that no test loads, the absolute one
        // matches those that are loaded.
        include: ["src/lib/**", "src/modules/**", "../ui/src/**", `${designSystem}/**`],
        // clients/ui is outside this package's root.
        allowExternal: true,
        exclude: ["**/*.gen.ts", "**/*.test.{ts,tsx}", "**/*.stories.tsx", "**/*.json", "**/*.css"],
        reporter: ["text", "html", "lcov"],
        thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
      },
    },
  }),
);
