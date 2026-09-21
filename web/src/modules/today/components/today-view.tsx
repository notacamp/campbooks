/**
 * TodayView — the assistant's desk surface.
 *
 * Presentational, prop-driven. Renders:
 *   1. Greeting + Scout brief (Visor + data.greeting.brief)
 *   2. Needs-you worklist — ranked items with inline action + Undo
 *   3. Coming up — deadlines/events within ~3 days
 *   4. Handled — quiet trust-line of Scout's counts since last visit
 *
 * Acting on an item collapses it in-place (grid-rows animation) after a brief
 * "done" confirmation strip that carries an Undo. Undo cancels the collapse.
 * When the last item is gone → happy all-clear state (Visor + message).
 *
 * Reduced-motion: collapse is instant; no animation delays (JS + CSS both
 * respect prefers-reduced-motion).
 *
 * Ported from the approved prototype (rx-body.html § data-surface="today",
 * rx-style.css, rx-script.html).
 *
 * NOTE — Module-surface exception:
 * This file lives in modules/today/components/ but IS the design/presentation
 * layer for the Today surface. The module ESLint rules (forbid-elements,
 * forbid-component-props, jsx-no-literals) that guard business-logic files do
 * not apply here; they are suppressed below so the build stays clean.
 */

/* eslint-disable react/forbid-elements -- TodayView is the design layer for this surface; HTML elements are intentional here */
/* eslint-disable react/forbid-component-props -- className on lib/ui components is the design layer's prerogative */
/* eslint-disable react/jsx-no-literals -- UI copy lives at the surface layer; i18n applied later */

import {
  forwardRef,
  useState,
  useRef,
  useCallback,
  type HTMLAttributes,
} from "react";
import { cn } from "~/lib/utils";
import { Visor } from "~/lib/ui/visor/visor";
import { Signal, type Place } from "~/lib/ui/signal/signal";
import { Avatar } from "~/lib/ui/avatar/avatar";
import { Button } from "~/lib/ui/button/button";
import styles from "./today-view.module.css";

// ── Data types (exactly the serializer shape + overdue bool from e1) ──────────

export interface TodayAction {
  kind: string;
  label: string;
  primary: boolean;
}

export interface NeedsYouItem {
  id: string;
  source: "people" | "money" | "time";
  ref_id: string | number;
  verb: string;
  place: string;
  title: string;
  subtitle: string | null;
  read: string | null;
  due: string | null;
  draft: boolean | null;
  overdue: boolean;
  actions: TodayAction[];
}

export interface ComingUpItem {
  on: string | null;
  label: string;
  sub: string | null;
  place: string;
}

export interface HandledCounts {
  filed: number;
  matched: number;
  tucked: number;
  added: number;
  since: string;
}

export interface Greeting {
  name: string;
  date: string;
  brief: string;
}

export interface TodayData {
  greeting: Greeting;
  needs_you: NeedsYouItem[];
  coming_up: ComingUpItem[];
  handled: HandledCounts;
}

// ── Component API ─────────────────────────────────────────────────────────────

export interface TodayViewProps extends HTMLAttributes<HTMLElement> {
  data: TodayData;
  /**
   * Fires when the user acts on an item — caller owns the mutation. May return a
   * Promise: the card stays in its "Done" state and collapses only when it
   * resolves; if it REJECTS, the card rolls back and shows an inline error. A
   * plain `void` return is treated as immediate success (back-compatible).
   */
  onAction: (itemId: string, actionKey: string) => void | Promise<void>;
  /** Optional restore — fires when the user undoes a resolved item. */
  onUndo?: (itemId: string) => void;
  /** Launches the Skim overlay over the worklist. */
  onSkim: () => void;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Map the aggregator's place string to the Signal Place type. */
const toPlace = (place: string): Place => {
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

/** Capitalise first letter of a verb for the pill label. */
const verbLabel = (verb: string): string =>
  verb.length === 0 ? verb : verb[0]!.toUpperCase() + verb.slice(1);

/**
 * Format a date string (ISO) as a short human label for "Coming up" rows.
 * Returns "Today", "Tomorrow", "Mon 22", "Oct 30", etc.
 */
const formatUpDate = (iso: string | null): string => {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const target = new Date(d);
  target.setHours(0, 0, 0, 0);
  const diffDays = Math.round(
    (target.getTime() - today.getTime()) / 86_400_000,
  );
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Tomorrow";
  const day = d.toLocaleDateString("en-GB", { weekday: "short" });
  const date = d.getDate();
  const month = d.toLocaleDateString("en-GB", { month: "short" });
  return diffDays < 7 ? `${day} ${date}` : `${month} ${date}`;
};

/** True when the coming-up item is within 2 days — gets the "soon" styling. */
const isSoon = (iso: string | null): boolean => {
  if (!iso) return false;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return false;
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const target = new Date(d);
  target.setHours(0, 0, 0, 0);
  const diffDays = Math.round(
    (target.getTime() - today.getTime()) / 86_400_000,
  );
  return diffDays <= 2;
};

/**
 * Format the "Handled since" date:
 *   - today/yesterday → "today" / "yesterday"
 *   - within 7 days → day name in lowercase ("monday")
 *   - older → "Mon, Sep 14"
 */
const formatSince = (iso: string): string => {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const target = new Date(d);
  target.setHours(0, 0, 0, 0);
  const diffDays = Math.round(
    (today.getTime() - target.getTime()) / 86_400_000,
  );
  if (diffDays === 0) return "today";
  if (diffDays === 1) return "yesterday";
  if (diffDays < 7)
    return d
      .toLocaleDateString("en-GB", { weekday: "long" })
      .toLowerCase();
  return d.toLocaleDateString("en-GB", {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
};

/** Build the handled counts string, omitting zero values. */
const handledLine = (h: HandledCounts): string => {
  const parts: string[] = [];
  if (h.filed > 0)
    parts.push(`${h.filed} ${h.filed === 1 ? "email" : "emails"} filed`);
  if (h.matched > 0)
    parts.push(
      `${h.matched} ${h.matched === 1 ? "payment" : "payments"} matched`,
    );
  if (h.tucked > 0)
    parts.push(
      `${h.tucked} ${h.tucked === 1 ? "newsletter" : "newsletters"} tucked away`,
    );
  if (h.added > 0)
    parts.push(`${h.added} ${h.added === 1 ? "item" : "items"} added`);
  return parts.join(" · ");
};

/**
 * How long a resolved item shows its "Done · Undo" strip before it collapses —
 * i.e. the undo window. Kept under the route's post-success cache reconcile
 * (UNDO_RECONCILE_MS = 5000) so the strip is gone before the list reconciles.
 */
const UNDO_WINDOW_MS = 4000;

// ── Item card ─────────────────────────────────────────────────────────────────

interface ItemCardProps {
  item: NeedsYouItem;
  resolved: boolean;
  gone: boolean;
  /** True when the last action on this item failed and was rolled back. */
  failed: boolean;
  onAction: (itemId: string, actionKey: string) => void;
  onUndo: (itemId: string) => void;
}

const ItemCard = forwardRef<HTMLDivElement, ItemCardProps>(
  ({ item, resolved, gone, failed, onAction, onUndo }, ref) => {
    const place = toPlace(item.place || item.source);
    const avatarKind: "person" | "service" =
      item.source === "money" ? "service" : "person";

    // Money-domain items can have a null title; fall back to subtitle, then to
    // the primary action's own label (e.g. "Add statement"), then a generic —
    // so a card never renders as a blank row.
    const primaryAction = item.actions?.find((a) => a.primary) ?? item.actions?.[0];
    const displayTitle =
      (item.title && item.title.length > 0 ? item.title : null) ??
      (item.subtitle && item.subtitle.length > 0 ? item.subtitle : null) ??
      primaryAction?.label ??
      "Needs you";
    const showSub =
      item.subtitle != null &&
      item.subtitle.length > 0 &&
      item.subtitle !== displayTitle;
    // Derive initials from the first two words of the display title.
    const titleInitials =
      displayTitle
        .split(/\s+/)
        .filter(Boolean)
        .slice(0, 2)
        .map((w) => w[0]?.toUpperCase() ?? "")
        .join("") || "·";

    return (
      <div
        ref={ref}
        className={cn(styles.item, gone && styles.itemGone)}
        aria-hidden={gone}
      >
        <div className={styles.itemIn}>
          {resolved ? (
            <div className={styles.doneStrip} role="status">
              <span className={styles.doneGlyph} aria-label="Done">
                <span
                  className={styles.doneGlyphMark}
                  aria-hidden={true}
                />
              </span>
              <span className={styles.doneLabel}>Done</span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  onUndo(item.id);
                }}
              >
                Undo
              </Button>
            </div>
          ) : (
            <>
              <div className={styles.itemRow}>
                <Avatar
                  initials={titleInitials}
                  kind={avatarKind}
                  place={place}
                  size={36}
                  aria-hidden={true}
                />

                <div className={styles.who}>
                  <p
                    className={cn(
                      styles.whoTitle,
                      item.overdue && styles.whoTitleOverdue,
                    )}
                  >
                    {displayTitle}
                  </p>
                  {showSub && (
                    <p className={styles.whoSub}>{item.subtitle}</p>
                  )}
                  {item.read != null && item.read.length > 0 && (
                    <p className={styles.read}>
                      {item.read}
                      {item.draft === true && (
                        <>
                          {" "}
                          <span className={styles.readDraft}>
                            I&rsquo;ve drafted a reply.
                          </span>
                        </>
                      )}
                    </p>
                  )}
                </div>

                <div className={styles.side}>
                  <Signal.Pill place={place}>{verbLabel(item.verb)}</Signal.Pill>
                </div>
              </div>

              <div className={styles.acts}>
                {item.actions.map((action) => (
                  <Button
                    key={action.kind}
                    variant={action.primary ? "default" : "secondary"}
                    size="sm"
                    onClick={() => {
                      onAction(item.id, action.kind);
                    }}
                  >
                    {action.label}
                  </Button>
                ))}
                {failed && (
                  <span className={styles.actFailed} role="status">
                    Couldn&rsquo;t do that — restored.
                  </span>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    );
  },
);

ItemCard.displayName = "ItemCard";

// ── TodayView ─────────────────────────────────────────────────────────────────

/**
 * The Today surface: greeting, finite worklist, coming up, handled.
 *
 * Keeps local state only for the collapse animation (resolved + gone sets);
 * all data comes from `data` and mutations route through `onAction` / `onUndo`.
 */
export const TodayView = forwardRef<HTMLElement, TodayViewProps>(
  (
    {
      data,
      onAction,
      onUndo,
      onSkim,
      className,
      ...props
    },
    ref,
  ) => {
    const { greeting, needs_you, coming_up, handled } = data;

    // Items currently showing the done-strip (awaiting collapse).
    const [resolvedIds, setResolvedIds] = useState<ReadonlySet<string>>(
      new Set(),
    );
    // Items fully collapsed (grid-rows animation done).
    const [goneIds, setGoneIds] = useState<ReadonlySet<string>>(new Set());
    // Items whose last action failed and was rolled back (shows inline error).
    const [failedIds, setFailedIds] = useState<ReadonlySet<string>>(new Set());

    // Timers per item — cleared on Undo.
    const timersRef = useRef<Record<string, ReturnType<typeof setTimeout>>>({});
    // Items the user undid while their mutation was still in flight — so a late
    // success doesn't collapse a card the user has already restored.
    const undoneRef = useRef<Set<string>>(new Set());

    const handleAction = useCallback(
      (itemId: string, actionKey: string): void => {
        // Optimistically show the done-strip; a fresh action clears any prior
        // "undone" mark and inline error for this item.
        undoneRef.current.delete(itemId);
        setResolvedIds((prev) => new Set([...prev, itemId]));
        setFailedIds((prev) => {
          if (!prev.has(itemId)) return prev;
          const next = new Set(prev);
          next.delete(itemId);
          return next;
        });

        // Collapse the card once the mutation has actually succeeded — unless
        // the user hit Undo while it was in flight. The "Done · Undo" strip
        // stays for UNDO_WINDOW_MS, which is the undo window: a functional
        // affordance, so it holds regardless of reduced-motion (the collapse
        // ANIMATION is CSS-gated on prefers-reduced-motion). Kept comfortably
        // under the route's post-success reconcile (UNDO_RECONCILE_MS = 5s) so
        // the strip is already gone before the cache reconciles.
        const startCollapse = (): void => {
          if (undoneRef.current.has(itemId)) {
            undoneRef.current.delete(itemId);
            return;
          }
          timersRef.current[itemId] = setTimeout(() => {
            setGoneIds((prev) => new Set([...prev, itemId]));
            delete timersRef.current[itemId];
          }, UNDO_WINDOW_MS);
        };

        // The mutation failed: restore the card and surface a quiet inline note
        // that auto-clears, so a failure never looks like a success.
        const rollback = (): void => {
          setResolvedIds((prev) => {
            const next = new Set(prev);
            next.delete(itemId);
            return next;
          });
          setFailedIds((prev) => new Set([...prev, itemId]));
          window.setTimeout(() => {
            setFailedIds((prev) => {
              if (!prev.has(itemId)) return prev;
              const next = new Set(prev);
              next.delete(itemId);
              return next;
            });
          }, 4000);
        };

        // Normalises a `void` return (immediate success) and a Promise alike.
        Promise.resolve(onAction(itemId, actionKey)).then(startCollapse, rollback);
      },
      [onAction],
    );

    const handleUndo = useCallback(
      (itemId: string): void => {
        // Mark undone so an in-flight mutation that resolves later won't collapse
        // the card we're restoring.
        undoneRef.current.add(itemId);
        const timer = timersRef.current[itemId];
        if (timer !== undefined) {
          clearTimeout(timer);
          delete timersRef.current[itemId];
        }
        setResolvedIds((prev) => {
          const next = new Set(prev);
          next.delete(itemId);
          return next;
        });
        setGoneIds((prev) => {
          const next = new Set(prev);
          next.delete(itemId);
          return next;
        });
        onUndo?.(itemId);
      },
      [onUndo],
    );

    // Active count: items not yet resolved or gone.
    const activeCount = needs_you.filter(
      (item) => !resolvedIds.has(item.id) && !goneIds.has(item.id),
    ).length;

    // All-clear once every item has been gone'd (or the list was empty).
    const allClear =
      needs_you.length === 0 ||
      needs_you.every((item) => goneIds.has(item.id));

    // Format the greeting date (ISO → "Monday 21 September").
    const greetingDate = (() => {
      const d = new Date(greeting.date);
      if (Number.isNaN(d.getTime())) return greeting.date;
      return d.toLocaleDateString("en-GB", {
        weekday: "long",
        day: "numeric",
        month: "long",
      });
    })();

    return (
      <section
        ref={ref}
        className={cn(className)}
        aria-label="Today"
        {...props}
      >
        {/* Greeting */}
        <p className={styles.kick}>{greetingDate}</p>
        <h1 className={styles.heading}>
          Good morning, {greeting.name}.
        </h1>
        <div className={styles.brief}>
          <Visor
            state="reading"
            size={32}
            gaze={true}
            className={styles.briefVisor}
            label="Scout"
          />
          <p className={styles.briefText}>{greeting.brief}</p>
        </div>

        {/* Needs you */}
        <div className={styles.sech} role="heading" aria-level={2}>
          <span>Needs you</span>
          <span
            className={styles.sechCount}
            aria-live="polite"
            aria-atomic="true"
          >
            {activeCount > 0 ? `· ${activeCount}` : null}
          </span>
          <span className={styles.sechLine} aria-hidden={true} />
          <Button variant="secondary" size="sm" onClick={onSkim}>
            Skim these
          </Button>
        </div>

        <div className={styles.work}>
          {needs_you.map((item) => (
            <ItemCard
              key={item.id}
              item={item}
              resolved={resolvedIds.has(item.id)}
              gone={goneIds.has(item.id)}
              failed={failedIds.has(item.id)}
              onAction={handleAction}
              onUndo={handleUndo}
            />
          ))}

          {/* All-clear */}
          {allClear && (
            <div
              className={styles.allClear}
              role="status"
              aria-live="polite"
              aria-atomic="true"
            >
              <Visor state="happy" size={64} label="Scout, happy" />
              <p className={styles.allClearTitle}>
                You&rsquo;re clear for today.
              </p>
              <p className={styles.allClearSub}>
                Nothing else needs you. I&rsquo;ll keep watching and surface
                anything new here.
              </p>
            </div>
          )}
        </div>

        {/* Coming up */}
        {coming_up.length > 0 && (
          <>
            <div className={styles.sech} role="heading" aria-level={2}>
              <span>Coming up</span>
              <span className={styles.sechLine} aria-hidden={true} />
            </div>
            <div className={styles.up}>
              {coming_up.map((item, idx) => {
                const place = toPlace(item.place);
                const soon = isSoon(item.on);
                return (
                  <div
                    key={`${item.on ?? ""}-${idx}`}
                    className={styles.upRow}
                  >
                    <span
                      className={cn(styles.when, soon && styles.whenSoon)}
                    >
                      {formatUpDate(item.on)}
                    </span>
                    <span className={styles.what}>
                      {item.label}
                      {item.sub != null && item.sub.length > 0 && (
                        <small className={styles.whatSub}>{item.sub}</small>
                      )}
                    </span>
                    <Signal.Pill place={place}>{item.place}</Signal.Pill>
                  </div>
                );
              })}
            </div>
          </>
        )}

        {/* Handled */}
        <div className={styles.sech} role="heading" aria-level={2}>
          <span>Handled since {formatSince(handled.since)}</span>
          <span className={styles.sechLine} aria-hidden={true} />
        </div>
        <div className={styles.handled}>
          <Visor
            state="watching"
            size={22}
            className={styles.handledVisor}
            label="Scout"
          />
          <span>
            {handledLine(handled)
              .split(/(\d+)/)
              .map((part, i) =>
                /^\d+$/.test(part) ? (
                  <strong key={i} className={styles.handledCount}>
                    {part}
                  </strong>
                ) : (
                  <span key={i}>{part}</span>
                ),
              )}
          </span>
          <span className={styles.handledFine}>
            Nothing was sent, paid or deleted without you.
          </span>
        </div>
      </section>
    );
  },
);

TodayView.displayName = "TodayView";
