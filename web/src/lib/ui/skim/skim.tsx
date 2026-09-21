/**
 * Skim — a focused, one-card-at-a-time flow over an auto-curated set.
 *
 * Full-surface overlay: progress bar + one card + keyboard shortcuts (1–n, Esc).
 * After the last card → a "cleared" state with a happy Visor.
 *
 * Ported from the approved prototype (rx-script.html / rx-style.css).
 * Behaviour is identical; motion degrades to cross-fade under
 * `prefers-reduced-motion`.
 *
 * Constraints:
 *   - No raw hex literals; no `oklch(<number>…)` literals — colours from tokens.
 *   - TS strict; no `any`.
 *   - `forwardRef` to the overlay `<div>`.
 *   - `role="dialog"` + `aria-modal`; focus management on open/close.
 */
import {
  forwardRef,
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { cn } from "~/lib/utils";
import { Visor } from "~/lib/ui/visor/visor";
import { Signal, type Place } from "~/lib/ui/signal/signal";
import { Avatar } from "~/lib/ui/avatar/avatar";
import { Button } from "~/lib/ui/button/button";
import styles from "./skim.module.css";

// ── Public API types ──────────────────────────────────────────────────────────

export interface SkimAction {
  /** Stable key passed to `onAction` to identify the chosen action. */
  key: string;
  label: string;
  /** Button appearance — maps to the design system button variants. */
  kind?: "primary" | "secondary" | "ghost";
  /** Fly-out direction when this action is triggered. Default: "right". */
  dir?: "right" | "left" | "down";
}

export interface SkimCard {
  id: string;
  avatar: {
    initials?: string;
    email?: string;
    kind?: "person" | "service";
    place?: Place;
  };
  title: string;
  subtitle?: string;
  /** Scout's one-line read — rendered as `React.ReactNode` (may include `<b>`). */
  read: React.ReactNode;
  pill?: { place: Place; label: string };
  /** Optional "Scout's draft" preview block. */
  draft?: React.ReactNode;
  /** 1–3 actions. */
  actions: SkimAction[];
}

export interface SkimProps {
  open: boolean;
  cards: SkimCard[];
  /** Heading shown on the "cleared" screen. Default: "You're clear." */
  clearedTitle?: string;
  /** Sub-text on the "cleared" screen. */
  clearedSub?: string;
  /** Called when the user activates an action. Caller performs the real mutation. */
  onAction: (card: SkimCard, actionKey: string) => void;
  /** Called when Esc, the close button, or "Done" on the cleared screen is pressed. */
  onClose: () => void;
}

// ── Internal helpers ──────────────────────────────────────────────────────────

type FlyDir = "outR" | "outL" | "outD";

const DIR_MAP: Record<NonNullable<SkimAction["dir"]>, FlyDir> = {
  right: "outR",
  left: "outL",
  down: "outD",
};

const getFlyDir = (dir?: SkimAction["dir"]): FlyDir =>
  dir ? DIR_MAP[dir] : "outR";

const mapVariant = (
  kind?: SkimAction["kind"],
): "default" | "secondary" | "ghost" =>
  kind === "secondary" ? "secondary" : kind === "ghost" ? "ghost" : "default";

// ── Component ─────────────────────────────────────────────────────────────────

/**
 * Skim — full-surface focused card-review overlay.
 *
 * Renders `null` when `open` is `false`.
 * Resets internal state whenever `open` transitions from `false` to `true`.
 */
export const Skim = forwardRef<HTMLDivElement, SkimProps>(
  (
    {
      open,
      cards,
      clearedTitle = "You're clear.",
      clearedSub,
      onAction,
      onClose,
    },
    ref,
  ) => {
    // ── State ─────────────────────────────────────────────────────────────────

    const [index, setIndex] = useState(0);
    const [flyDir, setFlyDir] = useState<FlyDir | null>(null);

    // Guard: prevents double-action while a fly-out is in progress.
    const flyingRef = useRef(false);

    // Focus management: remember what had focus before opening.
    const overlayRef = useRef<HTMLDivElement | null>(null);
    const prevFocusRef = useRef<Element | null>(null);

    // Merge the forwarded ref with our internal ref.
    const setRef = useCallback(
      (node: HTMLDivElement | null) => {
        overlayRef.current = node;
        if (typeof ref === "function") {
          ref(node);
        } else if (ref) {
          ref.current = node;
        }
      },
      [ref],
    );

    // ── Reset on open ─────────────────────────────────────────────────────────

    useEffect(() => {
      if (!open) return;
      setIndex(0);
      setFlyDir(null);
      flyingRef.current = false;
    }, [open]);

    // ── Focus management ──────────────────────────────────────────────────────

    useEffect(() => {
      if (open) {
        prevFocusRef.current = document.activeElement;
        // Defer so the element is visible before focusing.
        requestAnimationFrame(() => {
          overlayRef.current?.focus();
        });
      } else {
        if (prevFocusRef.current instanceof HTMLElement) {
          prevFocusRef.current.focus();
        }
      }
    }, [open]);

    // ── Derived values ────────────────────────────────────────────────────────

    // `currentCard` is `undefined` once all cards are consumed (or for an empty deck).
    const currentCard: SkimCard | undefined = cards[index];
    const isCleared = currentCard === undefined;
    const displayIndex = Math.min(index + 1, cards.length);

    // ── Action handler ────────────────────────────────────────────────────────

    const handleAction = useCallback(
      (action: SkimAction) => {
        if (!currentCard || flyingRef.current) return;

        flyingRef.current = true;
        onAction(currentCard, action.key);

        const reduced = window.matchMedia(
          "(prefers-reduced-motion: reduce)",
        ).matches;
        const dir = getFlyDir(action.dir);
        const delay = reduced ? 0 : 380;

        if (!reduced) {
          setFlyDir(dir);
        }

        setTimeout(() => {
          setIndex((prev) => prev + 1);
          setFlyDir(null);
          flyingRef.current = false;
        }, delay);
      },
      [currentCard, onAction],
    );

    // ── Keyboard shortcuts ────────────────────────────────────────────────────

    useEffect(() => {
      if (!open) return;

      const onKeyDown = (e: KeyboardEvent): void => {
        if (e.key === "Escape") {
          e.preventDefault();
          onClose();
          return;
        }

        if (isCleared || !currentCard) return;

        const n = parseInt(e.key, 10);
        if (n >= 1) {
          const action = currentCard.actions[n - 1];
          if (action) {
            e.preventDefault();
            handleAction(action);
          }
        }
      };

      window.addEventListener("keydown", onKeyDown);
      return () => {
        window.removeEventListener("keydown", onKeyDown);
      };
    }, [open, isCleared, currentCard, onClose, handleAction]);

    // ── Render ────────────────────────────────────────────────────────────────

    if (!open) return null;

    return (
      <div
        ref={setRef}
        role="dialog"
        aria-modal="true"
        aria-label="Skim"
        tabIndex={-1}
        className={styles.overlay}
        style={{ outline: "none" } as CSSProperties}
      >
        {/* Top bar */}
        <div className={styles.top}>
          <span className={styles.count} aria-live="polite" aria-atomic="true">
            {displayIndex}&thinsp;/&thinsp;{cards.length}
          </span>

          <div className={styles.progress} role="progressbar" aria-valuemin={0} aria-valuemax={cards.length} aria-valuenow={displayIndex}>
            {cards.map((card, i) => (
              <i
                key={card.id}
                className={cn(
                  styles.seg,
                  i < index && styles.segDone,
                  i === index && !isCleared && styles.segCur,
                )}
                aria-hidden="true"
              />
            ))}
          </div>

          <button
            type="button"
            aria-label="Close skim"
            className={styles.closeBtn}
            onClick={onClose}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 14 14"
              aria-hidden="true"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            >
              <line x1="2" y1="2" x2="12" y2="12" />
              <line x1="12" y1="2" x2="2" y2="12" />
            </svg>
          </button>
        </div>

        {/* Stage */}
        <div className={styles.stage}>
          {/* Active card */}
          {!isCleared && currentCard && (
            <div
              key={`${index}-${currentCard.id}`}
              className={cn(
                styles.card,
                styles.enter,
                flyDir !== null && styles[flyDir],
              )}
            >
              {/* Who row */}
              <div className={styles.who}>
                <Avatar
                  initials={currentCard.avatar.initials}
                  email={currentCard.avatar.email}
                  kind={currentCard.avatar.kind ?? "person"}
                  place={currentCard.avatar.place ?? "none"}
                  size={44}
                />
                <div className={styles.whoText}>
                  <div className={styles.whoTitle}>{currentCard.title}</div>
                  {currentCard.subtitle && (
                    <div className={styles.whoSub}>{currentCard.subtitle}</div>
                  )}
                </div>
                {currentCard.pill && (
                  <span className={styles.pill}>
                    <Signal.Pill place={currentCard.pill.place}>
                      {currentCard.pill.label}
                    </Signal.Pill>
                  </span>
                )}
              </div>

              {/* Scout's read */}
              <div className={styles.read}>{currentCard.read}</div>

              {/* Optional draft preview */}
              {currentCard.draft && (
                <div className={styles.draft}>
                  <div className={styles.draftLabel}>
                    <Visor size={18} state="reading" aria-hidden={true} />
                    Scout&rsquo;s draft
                  </div>
                  {currentCard.draft}
                </div>
              )}

              {/* Actions */}
              <div className={styles.acts}>
                {currentCard.actions.map((action, i) => (
                  <Button
                    key={action.key}
                    type="button"
                    variant={mapVariant(action.kind)}
                    size="lg"
                    onClick={() => {
                      handleAction(action);
                    }}
                  >
                    {action.label}
                    <span className={styles.keyHint} aria-hidden="true">
                      {i + 1}
                    </span>
                  </Button>
                ))}
              </div>

              {/* Keyboard hint */}
              <div className={styles.hint} aria-hidden="true">
                Press 1&ndash;{currentCard.actions.length}, or use the buttons
                &middot; Esc to leave
              </div>
            </div>
          )}

          {/* Cleared state */}
          {isCleared && (
            <div className={styles.end}>
              <Visor size={84} state="happy" label="All clear" />
              <strong className={styles.endTitle}>{clearedTitle}</strong>
              {clearedSub && (
                <span className={styles.endSub}>{clearedSub}</span>
              )}
              <Button type="button" variant="secondary" size="lg" onClick={onClose}>
                Done
              </Button>
            </div>
          )}
        </div>
      </div>
    );
  },
);

Skim.displayName = "Skim";
