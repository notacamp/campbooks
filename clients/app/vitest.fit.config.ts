import { playwright } from "@vitest/browser-playwright";
import { defineConfig, mergeConfig } from "vitest/config";

import viteConfig from "./vite.config.ts";

// Copy fits in every language (`pnpm test:fit`): every story rendered in the
// "longest" pseudo-locale at phone and desktop widths, in headless Chromium,
// with nothing of our copy cut off (test/fit/stories.fit.tsx).
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      name: "fit",
      include: ["test/fit/**/*.fit.tsx"],
      testTimeout: 30_000,
      browser: {
        enabled: true,
        headless: true,
        // Animations off, so every story is measured in its final layout.
        provider: playwright({ contextOptions: { reducedMotion: "reduce" } }),
        instances: [{ browser: "chromium" }],
      },
    },
  }),
);
