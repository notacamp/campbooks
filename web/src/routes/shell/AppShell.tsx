/**
 * AppShell — the assistant-first shell layout.
 *
 * Desktop (≥ 680px):
 *   Sidebar (220px) | main
 *                       Route content (flex-1, scrollable)
 *                       ScoutBar (sticky bottom)
 *
 * Mobile (< 680px):
 *   main
 *     Route content (flex-1, scrollable)
 *     ScoutBar
 *     Bottom tab bar (4 items)
 *
 * The 680px threshold matches the `md` Tailwind breakpoint (768px is default;
 * we use the `md` prefix loosely here via a custom inline check). Because
 * Tailwind v4's breakpoints are the same as v3 by default, we use `md:` (768px)
 * as the closest available breakpoint to the spec's 680px.
 */
import { type FC } from "react";
import { Outlet } from "@tanstack/react-router";
import { Clock, Inbox, BookOpen, Calendar } from "lucide-react";
import { Sidebar } from "./Sidebar";
import { ScoutBar } from "./ScoutBar";
import { NavItem } from "./NavItem";
import { cn } from "~/lib/utils";

const MOBILE_NAV = [
  { to: "/today",    label: "Today",    Icon: Clock      },
  { to: "/inbox",   label: "Inbox",    Icon: Inbox      },
  { to: "/books",   label: "Books",    Icon: BookOpen   },
  { to: "/calendar",label: "Calendar", Icon: Calendar   },
] as const;

export const AppShell: FC = () => (
  <div className="flex h-screen overflow-hidden bg-ground">
    {/* Desktop sidebar — hidden on mobile */}
    <Sidebar />

    {/* Main area */}
    <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
      {/* Route content — takes all remaining height */}
      <main
        className="flex-1 overflow-y-auto overflow-x-hidden"
        id="main-content"
      >
        <Outlet />
      </main>

      {/* Scout bar — always visible above the fold */}
      <ScoutBar />

      {/* Mobile bottom tab bar — shown only below md */}
      <nav
        className={cn(
          "flex md:hidden items-stretch border-t border-line bg-ground",
          "h-[52px]",
        )}
        aria-label="Main navigation"
      >
        {MOBILE_NAV.map(({ to, label, Icon }) => (
          <NavItem key={to} to={to} label={label} Icon={Icon} mobile />
        ))}
      </nav>
    </div>
  </div>
);
