import { defineConfig } from "vitest/config";

// The mail frame under the desktop and web page policies, in WebKit
// (campbooks-cloud#199): a Node test driving Playwright's WebKit, so it runs
// on its own (`pnpm test:csp`) once WebKit is installed, never with the jsdom
// suite.
export default defineConfig({
  test: {
    name: "webkit",
    environment: "node",
    include: ["test/webkit/**/*.webkit.ts"],
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
