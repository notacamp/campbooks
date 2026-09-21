/**
 * SettingsLayout — the two-pane settings shell.
 *
 * This component renders ONLY the settings sub-navigation + a content outlet.
 * It must NOT re-render the app shell (Sidebar/ScoutBar); those are already
 * provided by the `_auth` layout that wraps this route.
 *
 * Layout:
 *   Desktop (≥ md): sub-nav (200px) | content (flex-1, scrollable)
 *   Mobile (< md):  sub-nav (horizontal scroll strip) / content (stacked)
 *
 * The `<Outlet />` renders the active settings page component.
 */
import { type FC } from "react";
import { Outlet } from "@tanstack/react-router";
import { SettingsNav } from "./SettingsNav";
import { cn } from "~/lib/utils";

export const SettingsLayout: FC = () => (
  <div
    className="flex flex-col md:flex-row h-full overflow-hidden"
    data-testid="settings.layout"
  >
    {/* Left sub-nav — horizontal strip on mobile, sidebar on desktop */}
    <SettingsNav />

    {/* Content area — scrollable */}
    <main
      className={cn(
        "flex-1 min-w-0 overflow-y-auto overflow-x-hidden",
        "p-6",
      )}
      id="settings-content"
    >
      <Outlet />
    </main>
  </div>
);
