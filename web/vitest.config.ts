import { defineConfig } from "vitest/config";

// Note: vitest ships its own vite dependency; importing plugins from the outer
// vite causes TypeScript type conflicts. The ~ alias is resolved here directly.
// CSS imports from @tailwindcss/vite are a no-op in tests (jsdom ignores styles).
export default defineConfig({
  resolve: {
    alias: {
      "~": new URL("./src", import.meta.url).pathname,
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    globals: true,
    include: ["src/**/*.test.{ts,tsx}", "src/**/*.spec.{ts,tsx}"],
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      exclude: [
        "src/lib/ui/**",
        "src/**/*.stories.{ts,tsx}",
        "src/test/**",
        "src/**/*.d.ts",
      ],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 80,
      },
    },
  },
});
