/**
 * TodayPage — the assistant's desk surface at /today.
 *
 * Connects TodayView (design layer) to GET /api/app/today via useTodayQuery.
 * Manages the Skim overlay over data.needs_you.
 *
 * States: loading skeleton / error+retry / populated view.
 * Callbacks:
 *   onSkim()              — opens <Skim> over data.needs_you (working).
 *   onAction(id, key)     — dispatches to people/time/money endpoints
 *                           with optimistic removal + snapshot/rollback.
 *   onUndo?(id)           — calls the inverse people action (archive→unarchive
 *                           etc.) or invalidates Today for time/money items.
 *
 * Mirrors the Inbox pattern (useQuery → states → view).
 */
import { type FC, useState, useCallback, useRef } from "react";
import { createRoute, useNavigate } from "@tanstack/react-router";
import type { AnyRoute } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { RotateCcw } from "lucide-react";
import { Visor, Skim, type SkimCard, type SkimAction, type Place } from "~/lib/ui";
import { cn } from "~/lib/utils";
import { TodayView } from "~/modules/today/components";
import type { NeedsYouItem } from "~/modules/today/components";
import { useTodayQuery, todayKeys } from "~/modules/today/api";
import {
  useTodayActionMutation,
  useTodayUndoMutation,
  type ActionCall,
  type TodayActionHints,
} from "~/modules/today/api/use-today-action-mutation";

// ── Helpers ───────────────────────────────────────────────────────────────────

const toSkimPlace = (place: string): Place => {
  switch (place.toLowerCase()) {
    case "inbox":
    case "people":
      return "people";
    case "money":
      return "money";
    case "time":
    case "calendar":
      return "time";
    case "paper":
      return "paper";
    case "now":
      return "now";
    default:
      return "none";
  }
};

const titleInitials = (title: string | null | undefined): string =>
  (title ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? "")
    .join("");

const verbLabel = (verb: string | null | undefined): string =>
  !verb ? "" : (verb[0]?.toUpperCase() ?? "") + verb.slice(1);

/** Map a NeedsYouItem to a SkimCard for the Skim overlay. */
const toSkimCard = (item: NeedsYouItem): SkimCard => {
  const place = toSkimPlace(item.place || item.source);
  const itemActions = item.actions ?? [];
  const actions: SkimAction[] =
    itemActions.length > 0
      ? itemActions.map((a) => ({
          key: a.kind,
          label: a.label,
          kind: a.primary ? "primary" : "secondary",
          dir: a.primary ? "right" : ("down" as const),
        }))
      : [{ key: "skip", label: "Skip", kind: "ghost" as const, dir: "right" as const }];

  return {
    id: item.id,
    avatar: {
      initials: titleInitials(item.title),
      kind: item.source === "money" ? "service" : "person",
      place,
    },
    title:
      item.title ??
      item.actions?.find((a) => a.primary)?.label ??
      item.actions?.[0]?.label ??
      "Untitled",
    subtitle: item.subtitle ?? undefined,
    read: item.read ?? "",
    pill: { place, label: verbLabel(item.verb) },
    draft: item.draft === true ? "Scout has drafted a reply." : undefined,
    actions,
  };
};

// ── Loading skeleton ───────────────────────────────────────────────────────────

const TodaySkeleton: FC = () => (
  <div
    data-testid="today.page.loading"
    className="flex flex-col gap-4 p-6 max-w-2xl mx-auto w-full animate-pulse"
    aria-busy="true"
    aria-label="Loading today"
  >
    <div className="h-3.5 w-24 bg-surface-1 rounded-cb-1" />
    <div className="h-7 w-64 bg-surface-1 rounded-cb-1" />
    <div className="h-14 bg-surface-1 rounded-cb-1 mt-2" />
    <div className="h-3 w-32 bg-surface-1 rounded-cb-1 mt-4" />
    {[0, 1, 2].map((i) => (
      <div key={i} className="h-24 bg-surface-1 rounded-cb-1" />
    ))}
  </div>
);

// ── TodayPage ─────────────────────────────────────────────────────────────────

export const TodayPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useTodayQuery();
  const [skimOpen, setSkimOpen] = useState(false);
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const actionMutation = useTodayActionMutation();
  const undoMutation = useTodayUndoMutation();

  // Map of itemId → the action's undo call (stashed on a successful action).
  const undoMapRef = useRef<Map<string, ActionCall>>(new Map());

  const handleAction = useCallback(
    (itemId: string, actionKey: string): Promise<void> => {
      const item = data?.needs_you.find((i) => i.id === itemId);
      if (!item) return Promise.resolve();
      const action = item.actions.find((a) => a.kind === actionKey) as
        | TodayActionHints
        | undefined;

      // Navigational action (no endpoint — e.g. money "review") → open the surface.
      if (!action?.endpoint) {
        const dest =
          item.source === "money"
            ? "/books"
            : item.source === "time"
              ? "/calendar"
              : "/inbox";
        void navigate({ to: dest });
        return Promise.resolve();
      }

      // Fire the self-describing call. Return the promise so TodayView can await
      // it (resolve → collapse, reject → rollback). Stash the undo for onUndo.
      return actionMutation
        .mutateAsync({
          endpoint: action.endpoint,
          method: action.method ?? "POST",
          body: action.body ?? null,
        })
        .then(() => {
          if (action.undo) undoMapRef.current.set(itemId, action.undo);
        });
    },
    [data, actionMutation, navigate],
  );

  const handleUndo = useCallback(
    (itemId: string): void => {
      const undo = undoMapRef.current.get(itemId);
      if (undo) {
        undoMapRef.current.delete(itemId);
        undoMutation.mutate(undo);
      } else {
        // No inverse for this action (e.g. "paid" is one-way) — refetch.
        void queryClient.invalidateQueries({ queryKey: todayKeys.all });
      }
    },
    [undoMutation, queryClient],
  );

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div className="h-full overflow-y-auto">
        <TodaySkeleton />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="today.page.error"
        className="flex flex-col items-center justify-center h-full gap-3 p-6 text-center"
      >
        <Visor state="asleep" size={32} label="Error loading today" />
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
          data-testid="today.page.retry"
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </div>
    );
  }

  const skimCards = data.needs_you.map(toSkimCard);

  return (
    <div
      data-testid="today.page.root"
      className="h-full overflow-y-auto"
    >
      <TodayView
        data={data}
        onAction={handleAction}
        onUndo={handleUndo}
        onSkim={() => setSkimOpen(true)}
        className="max-w-2xl w-full mx-auto p-6"
      />
      <Skim
        open={skimOpen}
        cards={skimCards}
        clearedTitle="All clear."
        clearedSub="Nothing else needs you today."
        onAction={(card, actionKey) => {
          handleAction(card.id, actionKey);
        }}
        onClose={() => setSkimOpen(false)}
      />
    </div>
  );
};

// ── Route builder ─────────────────────────────────────────────────────────────

export const buildTodayRoutes = (parent: AnyRoute): AnyRoute[] => [
  createRoute({
    getParentRoute: () => parent,
    path: "/today",
    component: TodayPage,
  }),
];
