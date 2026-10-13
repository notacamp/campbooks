import { fileURLToPath, URL } from "node:url";

import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig, mergeConfig } from "vitest/config";

import viteConfig from "./vite.config.ts";

// Runs every story as a test in headless Chromium (`pnpm test-storybook`):
// each story must render without errors, pass its play function, and pass
// axe (parameters.a11y.test = "error" in .storybook/preview.tsx).
export default mergeConfig(
  viteConfig,
  defineConfig({
    plugins: [storybookTest({ configDir: fileURLToPath(new URL(".storybook", import.meta.url)) })],
    test: {
      name: "storybook",
      browser: {
        enabled: true,
        headless: true,
        provider: playwright({}),
        instances: [{ browser: "chromium" }],
      },
    },
  }),
);
