import type { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet, useNavigate, useParams } from "@tanstack/react-router";
import { Suspense, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { readPref } from "~/lib/storage/prefs";
import { CommandMenu, ShortcutSheet, Text, ThemeProvider } from "~/lib/ui";

import { SessionGate } from "../session/SessionGate";

import { NotFound } from "./not-found";

export interface RouterContext {
  queryClient: QueryClient;
}

function RootLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { workspaceId } = useParams({ strict: false });
  const [sheetOpen, setSheetOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);

  useShortcut("sheet.open", () => {
    setMenuOpen(false);
    setSheetOpen((open) => !open);
  });
  useShortcut("command.open", () => {
    setSheetOpen(false);
    setMenuOpen((open) => !open);
  });
  useShortcut("nav.home", () => {
    void navigate({ to: "/" });
  });

  return (
    // The accent follows the open workspace; outside one, the last one opened.
    <ThemeProvider workspaceId={workspaceId ?? readPref("workspace") ?? undefined}>
      <Suspense fallback={<Text tone="tertiary">{t("loading")}</Text>}>
        <SessionGate>
          <Outlet />
        </SessionGate>
      </Suspense>
      <ShortcutSheet
        open={sheetOpen}
        onClose={() => {
          setSheetOpen(false);
        }}
      />
      <CommandMenu
        open={menuOpen}
        onClose={() => {
          setMenuOpen(false);
        }}
      />
    </ThemeProvider>
  );
}

export const rootRoute = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
  notFoundComponent: NotFound,
});
