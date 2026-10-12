import type { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet } from "@tanstack/react-router";
import { Suspense } from "react";
import { useTranslation } from "react-i18next";

import { NotFound } from "./not-found";

export interface RouterContext {
  queryClient: QueryClient;
}

function RootLayout() {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-dvh flex-col bg-surface text-ink">
      <header className="border-b border-line px-4 py-3">
        <span className="font-semibold">{t("appName")}</span>
      </header>
      <main className="flex-1 px-4 py-6">
        <Suspense fallback={<p>{t("loading")}</p>}>
          <Outlet />
        </Suspense>
      </main>
    </div>
  );
}

export const rootRoute = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
  notFoundComponent: NotFound,
});
