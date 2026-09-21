/**
 * main.tsx — app composition root.
 *
 * Provider nesting (outermost → innermost):
 *   ThemeProvider (in RootLayout, via router)  dark-first, class strategy
 *   └─ QueryClientProvider                     TanStack Query — server state
 *      └─ IntlProvider                         react-intl — i18n
 *         └─ RouterProvider                    TanStack Router — routing
 *
 * Boot sequence:
 *   1. mountModules()  — register extension contributions before first render
 *   2. initObservability() — wire error-tracking sink to the logger
 *   3. Render
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { QueryClientProvider } from "@tanstack/react-query";
import { createQueryClient } from "~/lib/api";
import { IntlProvider } from "~/lib/i18n";
import { initObservability } from "~/lib/observability";
import { mountModules } from "~/modules";
import { router } from "~/router";
import "./styles/globals.css";

const queryClient = createQueryClient();

// Register module contributions and wire the observability sink before render.
mountModules();
initObservability();

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error(
    "Root element #root not found. Check index.html has <div id=\"root\">.",
  );
}

createRoot(rootElement).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <IntlProvider>
        <RouterProvider router={router} />
      </IntlProvider>
    </QueryClientProvider>
  </StrictMode>,
);
