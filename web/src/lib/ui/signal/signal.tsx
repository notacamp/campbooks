/**
 * Signal — the five-place semantic colour system (dots, labels, pills).
 *
 * Three sub-components exposed as properties of the `Signal` namespace object:
 *   Signal.Dot   — tiny rounded square in the place colour
 *   Signal.Label — neutral chip with a leading dot (identity)
 *   Signal.Pill  — tinted status chip (state)
 *
 * Ported from `.lbl` / `.pill` in ds-style.css.
 *
 * No raw hex. Signal hues come from `--cb-<place>` CSS variables already in
 * tokens.css and re-exposed as `--cb-<place>-text` for legible text.
 *
 * Rule 4 (DESIGN.md): signals appear only as dots + label chips. They must
 * always ship with a readable word (children), never colour alone.
 */
import { forwardRef, type HTMLAttributes, type CSSProperties } from "react";
import { cn } from "~/lib/utils";

// ── Shared types ──────────────────────────────────────────────────────────────

/** One of the five semantic places, or "none" for a neutral fallback. */
export type Place = "now" | "people" | "paper" | "money" | "time" | "none";

/** Resolves a place to its CSS variable for the signal hue. */
const placeHue = (place: Place): string =>
  place === "none" ? "var(--cb-t3)" : `var(--cb-${place})`;

/** Resolves a place to its readable text colour CSS variable. */
const placeText = (place: Place): string =>
  place === "none" ? "var(--cb-t2)" : `var(--cb-${place}-text)`;

// ── Dot ───────────────────────────────────────────────────────────────────────

export interface DotProps extends HTMLAttributes<HTMLSpanElement> {
  /** The semantic place this dot represents. */
  place: Place;
}

/**
 * A 7×7px rounded square in the place's signal hue.
 * Decorative by default (aria-hidden). Pass an aria-label for accessible use.
 */
export const Dot = forwardRef<HTMLSpanElement, DotProps>(
  ({ place, className, style, ...props }, ref) => (
    <span
      ref={ref}
      aria-hidden={props["aria-label"] ? undefined : true}
      className={cn("inline-block shrink-0 rounded-sm", className)}
      style={
        {
          width: "7px",
          height: "7px",
          backgroundColor: placeHue(place),
          ...style,
        } as CSSProperties
      }
      {...props}
    />
  ),
);

Dot.displayName = "Signal.Dot";

// ── Label ─────────────────────────────────────────────────────────────────────

export interface LabelProps extends HTMLAttributes<HTMLSpanElement> {
  /** The semantic place this label represents. */
  place: Place;
  /** Text inside the chip. Required — colour is never used alone. */
  children: React.ReactNode;
}

/**
 * Neutral pill chip with a leading signal dot (identity).
 * Maps to `.lbl` in ds-style.css.
 */
export const Label = forwardRef<HTMLSpanElement, LabelProps>(
  ({ place, children, className, style, ...props }, ref) => (
    <span
      ref={ref}
      className={cn(
        "inline-flex items-center gap-1.5 h-[22px] px-[9px] rounded-full",
        "text-[11.5px] font-[550] text-t3",
        "shadow-[inset_0_0_0_1px_var(--cb-line-2)]",
        className,
      )}
      style={style}
      {...props}
    >
      <Dot place={place} aria-hidden={true} />
      {children}
    </span>
  ),
);

Label.displayName = "Signal.Label";

// ── Pill ──────────────────────────────────────────────────────────────────────

export interface PillProps extends HTMLAttributes<HTMLSpanElement> {
  /** The semantic place this pill represents. */
  place: Place;
  /** Text inside the chip. Required — colour is never used alone. */
  children: React.ReactNode;
}

/**
 * Tinted status chip (state). Maps to `.pill` in ds-style.css.
 * Text is a mix of the signal hue and t1; background is a tinted wash.
 */
export const Pill = forwardRef<HTMLSpanElement, PillProps>(
  ({ place, children, className, style, ...props }, ref) => {
    const hue = placeHue(place);
    const text = placeText(place);
    return (
      <span
        ref={ref}
        className={cn(
          "inline-flex items-center gap-1.5 h-[22px] px-[9px] rounded-full",
          "text-[11.5px] font-[600]",
          className,
        )}
        style={
          {
            color: text,
            backgroundColor: `color-mix(in oklch, ${hue} 16%, transparent)`,
            ...style,
          } as CSSProperties
        }
        {...props}
      >
        <span
          aria-hidden={true}
          style={
            {
              display: "block",
              width: "6px",
              height: "6px",
              borderRadius: "99px",
              flexShrink: 0,
              backgroundColor: hue,
            } as CSSProperties
          }
        />
        {children}
      </span>
    );
  },
);

Pill.displayName = "Signal.Pill";

// ── Namespace object ──────────────────────────────────────────────────────────

/**
 * Signal — namespace for the three signal sub-components.
 *
 * Usage:
 *   <Signal.Dot place="people" />
 *   <Signal.Label place="now">Now</Signal.Label>
 *   <Signal.Pill place="money">Paid</Signal.Pill>
 */
export const Signal = {
  Dot,
  Label,
  Pill,
} as const;
