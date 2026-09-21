/**
 * SettingsNav — the settings sub-navigation registry and component.
 *
 * SETTINGS_NAV is the single source of truth that drives BOTH the left sub-nav
 * rendered here AND the route children registered in routes/settings/index.ts.
 * Adding a new section = add one entry here; the route follows from the same key.
 *
 * Design: same visual style as the app sidebar NavItem (13px font, rounded-cb-1,
 * surface-1 active/hover, accent underline on active).
 */
import { type FC, type ElementType } from "react";
import { Link } from "@tanstack/react-router";
import {
  User,
  Building2,
  Shield,
  Users,
  Sparkles,
  Brain,
  Inbox,
  Plug,
  Bell,
  CreditCard,
  Mail,
} from "lucide-react";
import { cn } from "~/lib/utils";

// ── Registry ──────────────────────────────────────────────────────────────────

export interface SettingsNavEntry {
  /** Unique key; also the relative route path segment. */
  key: string;
  /** Absolute URL for the Link. */
  to: string;
  label: string;
  Icon: ElementType;
}

/**
 * SETTINGS_NAV — the authoritative list of settings sections.
 * The `to` field must match the route path registered in routes/settings/index.ts
 * (`/settings/<key>`). `key` matches the route segment.
 */
export const SETTINGS_NAV: SettingsNavEntry[] = [
  { key: "account",       to: "/settings/account",       label: "Account",         Icon: User       },
  { key: "workspace",     to: "/settings/workspace",     label: "General",         Icon: Building2  },
  { key: "privacy",       to: "/settings/privacy",       label: "Data & privacy",  Icon: Shield     },
  { key: "members",       to: "/settings/members",       label: "Members",         Icon: Users      },
  { key: "ai",            to: "/settings/ai",            label: "AI",              Icon: Sparkles   },
  { key: "memory",        to: "/settings/memory",        label: "Scout's memory",  Icon: Brain      },
  { key: "inbox",         to: "/settings/inbox",         label: "Inbox settings",  Icon: Inbox      },
  { key: "integrations",  to: "/settings/integrations",  label: "Integrations",    Icon: Plug       },
  { key: "notifications", to: "/settings/notifications", label: "Notifications",   Icon: Bell       },
  { key: "plan",          to: "/settings/plan",          label: "Plan",            Icon: CreditCard },
  { key: "mailboxes",     to: "/settings/mailboxes",     label: "Mailboxes",       Icon: Mail       },
];

// ── Section groups for visual separation ──────────────────────────────────────

const ACCOUNT_KEYS = ["account", "workspace", "privacy"];
const TEAM_KEYS    = ["members"];
const AI_KEYS      = ["ai", "memory"];
const INBOX_KEYS   = ["inbox", "integrations", "notifications"];
const PLAN_KEYS    = ["plan", "mailboxes"];

const SECTION_ORDER = [ACCOUNT_KEYS, TEAM_KEYS, AI_KEYS, INBOX_KEYS, PLAN_KEYS];

// ── Component ─────────────────────────────────────────────────────────────────

export const SettingsNav: FC = () => (
  <nav
    className={cn(
      // Horizontal scroll on mobile ≤md, vertical on desktop
      "flex md:flex-col gap-0.5",
      "overflow-x-auto md:overflow-x-visible",
      "shrink-0",
      "md:w-[200px] md:min-h-0",
      "px-2 py-2 md:py-3",
      "border-b border-line md:border-b-0 md:border-r",
      "bg-ground",
    )}
    aria-label="Settings navigation"
    data-testid="settings.nav"
  >
    {SECTION_ORDER.map((group, gi) => {
      const entries = SETTINGS_NAV.filter((e) => group.includes(e.key));
      if (entries.length === 0) return null;
      return (
        <div
          key={gi}
          className={cn(
            "flex md:flex-col gap-0.5",
            gi > 0 && "md:mt-3 md:pt-3 md:border-t md:border-line",
            // horizontal spacing on mobile between groups
            gi > 0 && "ml-3 md:ml-0",
          )}
        >
          {entries.map((entry) => (
            <SettingsNavItem key={entry.key} entry={entry} />
          ))}
        </div>
      );
    })}
  </nav>
);

// ── Individual nav item ───────────────────────────────────────────────────────

interface SettingsNavItemProps {
  entry: SettingsNavEntry;
}

const SettingsNavItem: FC<SettingsNavItemProps> = ({ entry }) => {
  const { to, label, Icon } = entry;
  return (
    <Link
      to={to}
      className={cn(
        "flex items-center gap-2 px-3 py-1.5 rounded-cb-1",
        "text-[12.5px] font-medium text-t3 whitespace-nowrap",
        "hover:bg-surface-1 hover:text-t2",
        "transition-colors duration-[--cb-dur]",
        "[&.active]:bg-surface-1 [&.active]:text-t1",
      )}
      activeProps={{ className: "active" }}
      data-testid={`settings.nav.${entry.key}`}
    >
      <Icon size={14} strokeWidth={1.8} aria-hidden="true" className="shrink-0" />
      <span className="truncate">{label}</span>
    </Link>
  );
};
