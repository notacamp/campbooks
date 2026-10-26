import { fileURLToPath, URL } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const source = (file: string) => fileURLToPath(new URL(`src/${file}`, import.meta.url));

/** React and i18next come from the app, once (peer dependencies). */
const PEERS = /^(react|react-dom|i18next|react-i18next)(\/|$)/u;

// The published build (`pnpm build`): ES modules for the components and the
// shortcut engine, and every component stylesheet in one `dist/ui.css`. The
// token, base and font stylesheets are copied as they are
// (scripts/copy-assets.js), and the types come from tsconfig.build.json.
// Campbooks itself never builds this: it uses the source.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
    cssCodeSplit: false,
    lib: {
      entry: { index: source("index.ts"), shortcuts: source("shortcuts/index.ts") },
      formats: ["es"],
      fileName: (_format, entry) => `${entry}.js`,
      cssFileName: "ui",
    },
    rollupOptions: {
      external: (id) => PEERS.test(id),
    },
  },
});
