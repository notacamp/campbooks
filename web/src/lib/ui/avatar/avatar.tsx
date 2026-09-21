/**
 * Avatar — person or service contact mark.
 *
 * Ported from `.av` / `.av.svc` in ds-style.css.
 *   person  — round (border-radius 50%)
 *   service — square (rounded-cb-2, 8px radius)
 *
 * Background is a tinted wash of the place hue (30% mix with surface-1).
 * Text is a stronger mix of the hue (70% mix with t1).
 * Falls back to the accent hue when place is "none".
 *
 * Initials: pass `initials` directly, or derive from `email` when omitted.
 */
import { forwardRef, type HTMLAttributes, type CSSProperties } from "react";
import { cn } from "~/lib/utils";
import { type Place } from "~/lib/ui/signal/signal";

// ── Types ──────────────────────────────────────────────────────────────────────

export interface AvatarProps extends HTMLAttributes<HTMLSpanElement> {
  /** Explicit initials (up to 2 characters). */
  initials?: string;
  /** Email address — initials are derived when `initials` is not provided. */
  email?: string;
  /** Round for a person, square for a service/company. Default "person". */
  kind?: "person" | "service";
  /** Signal place for the tint colour. Default "none" (accent). */
  place?: Place;
  /** Size in px. Default 30. */
  size?: number;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Derive at most two uppercase initials from an email address. */
const initialsFromEmail = (email: string): string => {
  const local = email.split("@")[0] ?? "";
  // Split on dots, underscores, hyphens, plus signs
  const parts = local.split(/[._\-+]/).filter(Boolean);
  if (parts.length >= 2) {
    return `${(parts[0]?.[0] ?? "").toUpperCase()}${(parts[1]?.[0] ?? "").toUpperCase()}`;
  }
  return local.slice(0, 2).toUpperCase();
};

/** CSS for the tinted background from a place variable. */
const backgroundFromPlace = (place: Place): string => {
  const hue =
    place === "none" ? "var(--cb-accent)" : `var(--cb-${place})`;
  return `color-mix(in oklch, ${hue} 30%, var(--cb-s1))`;
};

/** CSS for the text colour from a place variable. */
const colorFromPlace = (place: Place): string => {
  const hue =
    place === "none" ? "var(--cb-accent)" : `var(--cb-${place})`;
  return `color-mix(in oklch, ${hue} 70%, var(--cb-t1))`;
};

// ── Component ──────────────────────────────────────────────────────────────────

/**
 * Contact mark showing initials in a tinted circle (person) or square (service).
 */
export const Avatar = forwardRef<HTMLSpanElement, AvatarProps>(
  (
    {
      initials,
      email,
      kind = "person",
      place = "none",
      size = 30,
      className,
      style,
      ...props
    },
    ref,
  ) => {
    const letters: string =
      initials?.slice(0, 2).toUpperCase() ??
      (email ? initialsFromEmail(email) : "?");

    const fontSize = Math.round(size * 0.37);

    const inlineStyle: CSSProperties = {
      width: `${size}px`,
      height: `${size}px`,
      fontSize: `${fontSize}px`,
      background: backgroundFromPlace(place),
      color: colorFromPlace(place),
      borderRadius: kind === "service" ? "var(--cb-r2)" : "50%",
      ...style,
    };

    return (
      <span
        ref={ref}
        className={cn(
          "inline-grid place-items-center shrink-0 select-none",
          "font-bold leading-none",
          className,
        )}
        style={inlineStyle}
        aria-label={props["aria-label"] ?? letters}
        {...props}
      >
        {letters}
      </span>
    );
  },
);

Avatar.displayName = "Avatar";
