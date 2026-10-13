import { defineConfig, mergeConfig } from "vitest/config";

import viteConfig from "./vite.config.ts";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      setupFiles: ["./test/setup.ts"],
      include: ["src/**/*.test.{ts,tsx}", "test/**/*.test.{ts,tsx}"],
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
        include: ["src/lib/**", "src/modules/**"],
        exclude: ["**/*.gen.ts", "**/*.test.{ts,tsx}", "**/*.stories.tsx", "**/*.json", "**/*.css"],
        reporter: ["text", "html", "lcov"],
        thresholds: { lines: 80, functions: 80, branches: 80, statements: 80 },
      },
    },
  }),
);
