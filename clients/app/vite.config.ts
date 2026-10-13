import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Set by `tauri dev` for iOS/Android so the device can reach the dev server.
const tauriDevHost = process.env["TAURI_DEV_HOST"];

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "~": fileURLToPath(new URL("src", import.meta.url)) },
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
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
