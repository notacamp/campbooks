import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/** A file of the design system's source (clients/ui/src). */
const designSystem = (file: string) => fileURLToPath(new URL(`../ui/src/${file}`, import.meta.url));

// Set by `tauri dev` for iOS/Android so the device can reach the dev server.
const tauriDevHost = process.env["TAURI_DEV_HOST"];

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: [
      { find: "~", replacement: fileURLToPath(new URL("src", import.meta.url)) },
      // @notacamp/ui is used from its source, so the app needs no package
      // build (the published build is clients/ui's `pnpm build`).
      { find: /^@notacamp\/ui$/, replacement: designSystem("index.ts") },
      { find: /^@notacamp\/ui\/shortcuts$/, replacement: designSystem("shortcuts/index.ts") },
      {
        find: /^@notacamp\/ui\/(tokens|tailwind-theme)\.css$/,
        replacement: designSystem("tokens/$1.css"),
      },
      { find: /^@notacamp\/ui\/(base|fonts)\.css$/, replacement: designSystem("styles/$1.css") },
    ],
    // One copy of React and i18next, whichever package's source imports them
    // (the design system's files resolve them from clients/ui).
    dedupe: ["react", "react-dom", "i18next", "react-i18next"],
  },
  // Keep Rust/Tauri errors visible in the terminal.
  clearScreen: false,
  server: {
    port: 5173,
    // Tauri's `devUrl` points at this exact port.
    strictPort: true,
    host: tauriDevHost ?? false,
    ...(tauriDevHost === undefined
      ? {}
      : { hmr: { protocol: "ws", host: tauriDevHost, port: 5174 } }),
    watch: { ignored: ["**/src-tauri/**"] },
    proxy: {
      // The server's web BFF (ADR 0004) on the dev server's origin, so the
      // session cookie and the same-origin checks work as in production.
      // Sign-in redirects back to CAMPBOOKS_PUBLIC_URL: run the server with
      // CAMPBOOKS_PUBLIC_URL=http://localhost:5173 (see clients/README.md).
      "/bff": {
        target: process.env["CAMPBOOKS_API_URL"] ?? "http://localhost:8080",
      },
      // The server's own identity provider (ADR 0014), when it runs in-process:
      // its issuer is CAMPBOOKS_PUBLIC_URL, so its endpoints live here too.
      "^/(oauth2|auth|\\.well-known)/": {
        target: process.env["CAMPBOOKS_API_URL"] ?? "http://localhost:8080",
      },
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
