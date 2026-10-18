import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";

import { DEFAULT_SHORTCUTS, ShortcutProvider, ShortcutRegistry } from "~/lib/shortcuts";
import { startScreenViews, startTelemetry } from "~/lib/telemetry";
import { UndoProvider } from "~/lib/ui";

import { i18n } from "./i18n";
import { MODULE_SHORTCUTS } from "./modules";
import { createAppRouter } from "./router";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      // One retry smooths over a blip without leaving a dead API spinning for long.
      retry: 1,
    },
  },
});

const router = createAppRouter(queryClient);

// The route template shown (`/w/$workspaceId/inbox`), never its address.
startTelemetry(() => router.state.matches.at(-1)?.routeId);

// Usage analytics: the screen shown after each navigation to another path,
// as its route template (`/w/:workspaceId/inbox`), never its address.
startScreenViews(
  (changed) => {
    router.subscribe("onResolved", ({ pathChanged }) => {
      if (pathChanged) {
        changed();
      }
    });
  },
  () => router.state.matches.at(-1)?.fullPath,
);

// One registry for the app: the defaults plus every module's operation
// shortcuts, conflict-checked once at startup.
const shortcuts = new ShortcutRegistry([...DEFAULT_SHORTCUTS, ...MODULE_SHORTCUTS]);

export function App() {
  return (
    <StrictMode>
      <I18nextProvider i18n={i18n}>
        <ShortcutProvider registry={shortcuts}>
          <UndoProvider>
            <QueryClientProvider client={queryClient}>
              <RouterProvider router={router} />
            </QueryClientProvider>
          </UndoProvider>
        </ShortcutProvider>
      </I18nextProvider>
    </StrictMode>
  );
}
