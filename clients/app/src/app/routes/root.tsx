import type { QueryClient } from "@tanstack/react-query";
import {
  createRootRouteWithContext,
  createRoute,
  Outlet,
  useNavigate,
  useParams,
} from "@tanstack/react-router";
import { Suspense, useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { ScoutChatHost, type OpenScoutChat } from "~/lib/scout-chat";
import { useShortcut } from "~/lib/shortcuts";
import { readPref } from "~/lib/storage/prefs";
import { CommandMenu, ShortcutSheet, Text, ThemeProvider } from "~/lib/ui";
import { useConversationSection } from "~/modules/search";

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
  // What had the focus when the menu opened: Scout's chat, asked from the
  // menu, gives it back there when it closes.
  const menuFrom = useRef<HTMLElement | null>(null);
  const openMenu = (open: boolean) => {
    if (open) {
      const active = document.activeElement;
      menuFrom.current = active instanceof HTMLElement && active !== document.body ? active : null;
    }
    setMenuOpen(open);
  };
  // Scout's chat of the open workspace, for "Ask Scout" in the menu (⌘K).
  const [scout, setScout] = useState<OpenScoutChat | undefined>();
  const hostScout = useCallback((open?: OpenScoutChat) => {
    setScout(() => open);
  }, []);

  // `/` searches the open workspace: in the menu on a desktop, the page on a phone.
  const openSearch = () => {
    setSheetOpen(false);
    if (workspaceId === undefined) {
      return;
    }
    if (typeof globalThis.matchMedia === "function" && globalThis.matchMedia(PHONE).matches) {
      setMenuOpen(false);
      void navigate({ to: "/w/$workspaceId/search", params: { workspaceId } });
    } else {
      openMenu(true);
    }
  };
  useShortcut("search.open", openSearch, { enabled: workspaceId !== undefined });

  useShortcut("sheet.open", () => {
    setMenuOpen(false);
    setSheetOpen((open) => !open);
  });
  useShortcut("command.open", () => {
    setSheetOpen(false);
    openMenu(!menuOpen);
  });
  useShortcut("nav.home", () => {
    void navigate({ to: "/" });
  });

  return (
    // The accent follows the open workspace; outside one, the last one opened.
    <ThemeProvider workspaceId={workspaceId ?? readPref("workspace") ?? undefined}>
      <Suspense fallback={<Text tone="tertiary">{t("loading")}</Text>}>
        <ScoutChatHost onOpener={hostScout}>
          <Outlet />
        </ScoutChatHost>
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
        useSection={useConversationSection}
        onAsk={
          scout === undefined
            ? undefined
            : (query) => {
                // A new chat about nothing; with words typed, asked at once.
                scout(null, {
                  from: menuFrom.current,
                  ask: query.trim() === "" ? undefined : query,
                });
              }
        }
      />
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
