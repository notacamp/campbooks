import type { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext, Outlet, useNavigate } from "@tanstack/react-router";
import { Suspense, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { CommandMenu, IconButton, KbdHint, ShortcutSheet, Spacer, Text } from "~/lib/ui";

import { NotFound } from "./not-found";

export interface RouterContext {
  queryClient: QueryClient;
}

function RootLayout() {
  const { t } = useTranslation();
  const { t: tShortcuts } = useTranslation("shortcuts");
  const navigate = useNavigate();
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
    <div className="flex min-h-dvh flex-col bg-surface-base text-primary">
      <header className="flex items-center gap-2 border-b border-subtle px-4 py-2">
        <Text as="span" weight="semibold">
          {t("appName")}
        </Text>
        <Spacer />
        <KbdHint shortcut="sheet.open" />
        <IconButton
          icon="keyboard"
          label={tShortcuts("sheet.open")}
          shortcut="sheet.open"
          onClick={() => {
            setSheetOpen(true);
          }}
        />
      </header>
      <main className="flex-1 px-4 py-6">
        <Suspense fallback={<Text tone="tertiary">{t("loading")}</Text>}>
          <Outlet />
        </Suspense>
      </main>
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
    </div>
  );
}

export const rootRoute = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
  notFoundComponent: NotFound,
});
