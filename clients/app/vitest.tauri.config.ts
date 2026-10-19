import { defineConfig } from "vitest/config";

// The mail frame in the real desktop shell (campbooks-cloud#237): a Node test
// driving a debug build of the Tauri app through `tauri-driver`, so it runs
// on its own (`pnpm test:tauri`, CI's `tauri-shell` job), never with the
// jsdom suite.
export default defineConfig({
  test: {
    name: "tauri",
    environment: "node",
    include: ["test/tauri/**/*.tauri.ts"],
    testTimeout: 120_000,
    hookTimeout: 200_000,
  },
});
