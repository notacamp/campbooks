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
      // LOCAL DEVELOPMENT ONLY. Stands in for the server's BFF so the web
      // client can be developed against `cargo run` without a session layer:
      // `/bff/api/v1/...` is forwarded to the API's `/api/v1/...` with no
      // credentials attached. The real BFF (httpOnly session cookie + OIDC,
      // ADR 0004) is served by campbooks-server and lands in a later task.
      "/bff": {
        target: process.env["CAMPBOOKS_API_URL"] ?? "http://localhost:8080",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/bff\/api\//, "/api/"),
      },
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
