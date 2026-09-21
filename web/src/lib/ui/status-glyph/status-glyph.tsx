/**
 * StatusGlyph — semantic status indicator with three states.
 *
 * Ported from `.st` in ds-style.css.
 *   open  — ring outline (nothing done yet)
 *   half  — half-filled circle (in progress)
 *   done  — filled circle with a check mark (complete)
 *
 * Coloured by `place` (one of the five signal hues) or neutral when "none".
 * Rendered as an inline SVG so it scales cleanly at any `size`.
 */
import { forwardRef, type SVGAttributes, type CSSProperties } from "react";
import { type Place } from "~/lib/ui/signal/signal";

// ── Types ──────────────────────────────────────────────────────────────────────

export type GlyphStatus = "open" | "half" | "done";

export interface StatusGlyphProps extends SVGAttributes<SVGSVGElement> {
  /** Visual state of the item. */
  status: GlyphStatus;
  /** Signal place for the colour. Defaults to "none" (neutral). */
  place?: Place;
  /** Size in px (both width and height). Default 16. */
  size?: number;
  /** Accessible label. When omitted the glyph is decorative (aria-hidden). */
  label?: string;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const placeColor = (place: Place): string =>
  place === "none" ? "var(--cb-t3)" : `var(--cb-${place})`;

// ── Component ──────────────────────────────────────────────────────────────────

/**
 * Inline SVG status glyph. Scales with `size`; colour from `place` token.
 */
export const StatusGlyph = forwardRef<SVGSVGElement, StatusGlyphProps>(
  (
    {
      status,
      place = "none",
      size = 16,
      label,
      style,
      ...props
    },
    ref,
  ) => {
    const color = placeColor(place);
    const r = size / 2;
    const cx = r;
    const cy = r;
    // Inner radius for the ring stroke
    const strokeW = Math.max(1.5, size * 0.1);
    const outerR = r - strokeW / 2;

    const a11yProps = label
      ? { role: "img" as const, "aria-label": label }
      : { "aria-hidden": true as const };

    const svgStyle: CSSProperties = {
      display: "inline-block",
      flexShrink: 0,
      verticalAlign: "middle",
      ...style,
    };

    return (
      <svg
        ref={ref}
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        fill="none"
        style={svgStyle}
        {...a11yProps}
        {...props}
      >
        {status === "open" && (
          <circle
            cx={cx}
            cy={cy}
            r={outerR}
            stroke={color}
            strokeWidth={strokeW}
          />
        )}

        {status === "half" && (
          <>
            {/* Background ring */}
            <circle
              cx={cx}
              cy={cy}
              r={outerR}
              stroke={color}
              strokeWidth={strokeW}
              opacity={0.3}
            />
            {/* Half-filled arc (top half) using a clip */}
            <clipPath id={`half-clip-${size}`}>
              <rect x={0} y={0} width={size} height={r} />
            </clipPath>
            <circle
              cx={cx}
              cy={cy}
              r={r - strokeW}
              fill={color}
              clipPath={`url(#half-clip-${size})`}
            />
          </>
        )}

        {status === "done" && (
          <>
            {/* Filled circle */}
            <circle cx={cx} cy={cy} r={r} fill={color} />
            {/* Check mark */}
            <polyline
              points={`${size * 0.28},${size * 0.52} ${size * 0.44},${size * 0.67} ${size * 0.72},${size * 0.35}`}
              stroke="var(--cb-bg)"
              strokeWidth={Math.max(1.25, size * 0.11)}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </>
        )}
      </svg>
    );
  },
);

StatusGlyph.displayName = "StatusGlyph";
