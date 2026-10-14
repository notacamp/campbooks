import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  createRoute,
  Outlet,
  useNavigate,
  useParams,
} from "@tanstack/react-router";
import { Suspense, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { readPref } from "~/lib/storage/prefs";
import { CommandMenu, ShortcutSheet, Text, ThemeProvider } from "~/lib/ui";
import { SearchOverlay } from "~/modules/search";

import { SessionGate } from "../session/SessionGate";

import { NotFound } from "./not-found";

export interface RouterContext {
  queryClient: QueryClient;
}

/** Below this width a floating panel is no use: search opens as a page (48rem, the shell's `md`). */
const PHONE = "(width < 48rem)";

function RootLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { workspaceId } = useParams({ strict: false });
  const [sheetOpen, setSheetOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [search, setSearch] = useState<{ open: boolean; query: string }>({
    open: false,
    query: "",
  });

  // `/` searches the open workspace: an overlay on a desktop, the page on a phone.
  const openSearch = (query = "") => {
    setMenuOpen(false);
    setSheetOpen(false);
    if (workspaceId === undefined) {
      return;
    }
    if (typeof globalThis.matchMedia === "function" && globalThis.matchMedia(PHONE).matches) {
      void navigate({
        to: "/w/$workspaceId/search",
        params: { workspaceId },
        search: query === "" ? {} : { q: query },
      });
    } else {
      setSearch({ open: true, query });
    }
  };
  useShortcut(
    "search.open",
    () => {
      openSearch();
    },
    { enabled: workspaceId !== undefined },
  );

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
        <Outlet />
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
        onSearch={workspaceId === undefined ? undefined : openSearch}
      />
      {workspaceId === undefined ? null : (
        <SearchOverlay
          workspaceId={workspaceId}
          open={search.open}
          initialQuery={search.query}
          onClose={() => {
            setSearch({ open: false, query: "" });
          }}
        />
      )}
    </ThemeProvider>
  );
}

export const rootRoute = createRootRouteWithContext<RouterContext>()({
  component: RootLayout,
  notFoundComponent: NotFound,
});

/**
 * Every page that needs a signed-in browser sits under this pathless layout,
 * which renders the session gate. Public pages (the sign-in screens) sit
 * beside it under the root.
 */
export const gatedRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "gated",
  component: function GatedLayout() {
    return (
      <SessionGate>
        <Outlet />
      </SessionGate>
    );
  },
});
