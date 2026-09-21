/**
 * NavItem — a single sidebar navigation entry.
 *
 * Shows an icon, label, and an optional count badge. Renders as a <Link>
 * from TanStack Router; active state is styled via aria-current.
 *
 * Mobile: renders as a tab item (icon only + label below).
 */
import { type FC, type ElementType } from "react";
import { Link } from "@tanstack/react-router";
import { cn } from "~/lib/utils";

export interface NavItemProps {
  to: string;
  label: string;
  Icon: ElementType;
  count?: number;
  mobile?: boolean;
}

export const NavItem: FC<NavItemProps> = ({ to, label, Icon, count, mobile = false }) => {
  if (mobile) {
    return (
      <Link
        to={to}
        className={cn(
          "flex flex-col items-center justify-center gap-0.5 flex-1 py-1 text-t3",
          "transition-colors duration-[--cb-dur]",
          "[&.active]:text-t1",
        )}
        activeProps={{ className: "active text-t1" }}
        aria-label={label}
      >
        <span className="relative">
          <Icon size={20} strokeWidth={1.8} aria-hidden="true" />
          {count != null && count > 0 && (
            <span
              className="absolute -top-1 -right-1.5 min-w-[14px] h-[14px] px-0.5 rounded-full bg-accent-cb text-[9px] font-bold text-white flex items-center justify-center"
              aria-label={`${count} unread`}
            >
              {count > 99 ? "99+" : count}
            </span>
          )}
        </span>
        <span className="text-[10px] font-medium leading-none">{label}</span>
      </Link>
    );
  }

  return (
    <Link
      to={to}
      className={cn(
        "flex items-center gap-2.5 px-3 py-2 rounded-cb-1 text-t2 text-[13px] font-medium",
        "hover:bg-surface-1 hover:text-t1 transition-colors duration-[--cb-dur]",
        "[&.active]:bg-surface-1 [&.active]:text-t1",
      )}
      activeProps={{ className: "active" }}
    >
      <Icon size={16} strokeWidth={1.8} aria-hidden="true" className="shrink-0" />
      <span className="flex-1 truncate">{label}</span>
      {count != null && count > 0 && (
        <span
          className="min-w-[18px] h-[18px] px-1 rounded-full bg-surface-2 text-t3 text-[10px] font-bold flex items-center justify-center tabular-nums"
          aria-label={`${count} items`}
        >
          {count > 99 ? "99+" : count}
        </span>
      )}
    </Link>
  );
};
