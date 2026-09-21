/**
 * Sidebar — the persistent navigation rail (≈220px wide).
 *
 * Contains:
 *   - Workspace mark: wordmark + Visor
 *   - Four nav items: Today, Inbox, Books, Calendar
 *   - Footer: Settings link + Scout status ("· Mistral, EU") + current user (→ /settings/account)
 *
 * Below 680px the sidebar is hidden and replaced by a bottom tab bar
 * rendered by AppShell (this component only renders at ≥ 680px).
 */
import { type FC, useCallback } from "react";
import { Clock, Inbox, BookOpen, Calendar, Settings, LogOut } from "lucide-react";
import { Link, useNavigate } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { Visor, Avatar } from "~/lib/ui";
import { useMe, clearToken } from "~/lib/api";
import { NavItem } from "./NavItem";
import { cn } from "~/lib/utils";

const NAV_ITEMS = [
  { to: "/today",    label: "Today",    Icon: Clock      },
  { to: "/inbox",   label: "Inbox",    Icon: Inbox      },
  { to: "/books",   label: "Books",    Icon: BookOpen   },
  { to: "/calendar",label: "Calendar", Icon: Calendar   },
] as const;

export const Sidebar: FC = () => {
  const { data: me } = useMe();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const handleSignOut = useCallback((): void => {
    clearToken();
    queryClient.clear();
    void navigate({ to: "/login" });
  }, [navigate, queryClient]);

  return (
    <aside
      className={cn(
        "hidden md:flex flex-col w-[220px] shrink-0",
        "bg-ground border-r border-line",
        "select-none",
      )}
      style={{ width: "220px" }}
    >
      {/* Workspace mark */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-line">
        <Visor size={20} state="watching" gaze label="Scout" />
        <span className="text-[14px] font-[650] text-t1 tracking-tight">
          campbooks
        </span>
      </div>

      {/* Nav */}
      <nav
        className="flex-1 flex flex-col gap-0.5 p-2 overflow-y-auto"
        aria-label="Main navigation"
      >
        {NAV_ITEMS.map(({ to, label, Icon }) => (
          <NavItem key={to} to={to} label={label} Icon={Icon} />
        ))}
      </nav>

      {/* Footer */}
      <div className="border-t border-line p-2 flex flex-col gap-1">
        {/* Settings — meta, not a place, so it sits in the footer above Scout status */}
        <NavItem to="/settings" label="Settings" Icon={Settings} />

        {/* Scout AI status */}
        <div className="flex items-center gap-1.5 px-3 py-1.5">
          <Visor size={14} state="watching" aria-hidden />
          <span className="text-[11px] text-t4 truncate">Mistral, EU</span>
        </div>

        {/* Current user */}
        {me?.user && (
          <Link
            to="/settings/account"
            className="flex items-center gap-2 px-3 py-1.5 rounded-cb-1 hover:bg-surface-1 transition-colors duration-[--cb-dur]"
            aria-label="Account settings"
          >
            <Avatar
              initials={(me.user.name ?? "?").slice(0, 2)}
              email={me.user.email}
              size={24}
              place="none"
              aria-hidden="true"
            />
            <span className="text-[12px] text-t2 truncate">{me.user.name ?? ""}</span>
          </Link>
        )}

        {/* Sign out */}
        <button
          type="button"
          onClick={handleSignOut}
          aria-label="Sign out"
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 w-full",
            "text-[11.5px] text-t4 hover:text-t2 hover:bg-surface-1",
            "transition-colors duration-[--cb-dur]",
          )}
          data-testid="sidebar.sign-out"
        >
          <LogOut size={13} strokeWidth={1.8} aria-hidden="true" />
          Sign out
        </button>
      </div>
    </aside>
  );
};
