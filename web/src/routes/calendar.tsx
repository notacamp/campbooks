/**
 * CalendarPage — the agenda surface at /calendar.
 *
 * Connects CalendarView (design layer) to GET /api/app/time via useCalendarQuery.
 * CalendarView is read-only: no action callbacks.
 *
 * States: loading skeleton / error+retry / populated agenda view.
 *
 * Mirrors the Inbox pattern (useQuery → states → view).
 */
import { type FC } from "react";
import { createRoute } from "@tanstack/react-router";
import type { AnyRoute } from "@tanstack/react-router";
import { RotateCcw } from "lucide-react";
import { Visor } from "~/lib/ui";
import { cn } from "~/lib/utils";
import { CalendarView } from "~/modules/calendar/components";
import { useCalendarQuery } from "~/modules/calendar/api";

// ── Loading skeleton ───────────────────────────────────────────────────────────

const CalendarSkeleton: FC = () => (
  <div
    data-testid="calendar.page.loading"
    className="flex flex-col gap-4 p-6 max-w-2xl mx-auto w-full animate-pulse"
    aria-busy="true"
    aria-label="Loading calendar"
  >
    {/* Day heading placeholder */}
    {[0, 1, 2].map((i) => (
      <div key={i} className="flex flex-col gap-2">
        <div className="h-5 w-24 bg-surface-1 rounded-cb-1" />
        <div className="h-16 bg-surface-1 rounded-cb-1" />
        {i === 0 && <div className="h-16 bg-surface-1 rounded-cb-1" />}
      </div>
    ))}
  </div>
);

// ── CalendarPage ──────────────────────────────────────────────────────────────

export const CalendarPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useCalendarQuery();

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div className="h-full overflow-y-auto">
        <CalendarSkeleton />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="calendar.page.error"
        className="flex flex-col items-center justify-center h-full gap-3 p-6 text-center"
      >
        <Visor state="asleep" size={32} label="Error loading calendar" />
        <p className="text-[13px] text-t2">
          {error instanceof Error ? error.message : "Something went wrong"}
        </p>
        <button
          type="button"
          onClick={() => refetch()}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "bg-surface-1 border border-line text-[12.5px] text-t2",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
          data-testid="calendar.page.retry"
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div
      data-testid="calendar.page.root"
      className="h-full overflow-y-auto"
    >
      <CalendarView
        data={data}
        className="max-w-2xl w-full mx-auto p-6"
      />
    </div>
  );
};

// ── Route builder ─────────────────────────────────────────────────────────────

export const buildCalendarRoutes = (parent: AnyRoute): AnyRoute[] => [
  createRoute({
    getParentRoute: () => parent,
    path: "/calendar",
    component: CalendarPage,
  }),
];
