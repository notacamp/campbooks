import { useTranslation } from "react-i18next";

import { cx } from "../../internal/cx";

import styles from "./Visor.module.css";

export const VISOR_STATES = [
  "idle",
  "reading",
  "thinking",
  "unsure",
  "happy",
  "alert",
  "asleep",
  "dizzy",
  "starry",
  "hearts",
] as const;
export type VisorState = (typeof VISOR_STATES)[number];

export interface VisorProps {
  /**
   * What the assistant is doing or feeling. Its eyes say it; motion stops under reduced motion.
   * `dizzy`: something failed on our side. `starry`: a big win. `hearts`: a warm moment.
   */
  state?: VisorState | undefined;
  /** sm 22px · md ×1.4 · lg ×2 · xl ×2.8 */
  size?: "sm" | "md" | "lg" | "xl" | undefined;
  /** Announce the state ("Campbooks is reading"). Decorative by default. */
  announce?: boolean | undefined;
  /** Colour the star and heart eyes (`--visor-tint-star`, `--visor-tint-heart`). Eyes are white by default. */
  tinted?: boolean | undefined;
  className?: string | undefined;
}

/**
 * An Archimedean spiral in a 24 x 24 box, drawn once: 2.5 turns, radius 0.6 to 9, so with the
 * stroke it stays inside the box (and the glass) with margin.
 */
const SPIRAL_PATH = Array.from({ length: 121 }, (_, i) => {
  const t = (i / 120) * Math.PI * 5;
  const r = 0.6 + t * (8.4 / (Math.PI * 5));
  return `${i === 0 ? "M" : "L"}${(12 + r * Math.cos(t)).toFixed(2)} ${(12 + r * Math.sin(t)).toFixed(2)}`;
}).join(" ");

/** A smiling eye: an upward arc, "^". */
const HAPPY_PATH = "M4.5 17C8 8.5 16 8.5 19.5 17";

const STAR_PATH =
  "M12 0.5C12.9 7.6 16.4 11.1 23.5 12 16.4 12.9 12.9 16.4 12 23.5 11.1 16.4 7.6 12.9 0.5 12 7.6 11.1 11.1 7.6 12 0.5Z";
const HEART_PATH =
  "M12 21.4l-1.5-1.3C5.4 15.4 2 12.3 2 8.5 2 5.4 4.4 3 7.5 3c1.7 0 3.4.8 4.5 2.1C13.1 3.8 14.8 3 16.5 3 19.6 3 22 5.4 22 8.5c0 3.8-3.4 6.9-8.5 11.5L12 21.4z";

type GlyphState = "happy" | "dizzy" | "starry" | "hearts";

const GLYPH_STATES: ReadonlySet<VisorState> = new Set(["happy", "dizzy", "starry", "hearts"]);

function Glyph({ state }: { state: GlyphState }) {
  return (
    <svg className={styles["glyph"]} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {state === "happy" ? (
        <path
          d={HAPPY_PATH}
          fill="none"
          stroke="currentColor"
          strokeWidth="4.4"
          strokeLinecap="round"
        />
      ) : state === "dizzy" ? (
        <path
          d={SPIRAL_PATH}
          fill="none"
          stroke="currentColor"
          strokeWidth="2.2"
          strokeLinecap="round"
        />
      ) : (
        <path d={state === "starry" ? STAR_PATH : HEART_PATH} fill="currentColor" />
      )}
    </svg>
  );
}

/** The assistant's face: a dark glass visor with two eyes. */
export function Visor({
  state = "idle",
  size = "sm",
  announce = false,
  tinted = false,
  className,
}: VisorProps) {
  const { t } = useTranslation("ui");
  const glyph = GLYPH_STATES.has(state) ? (state as GlyphState) : null;
  return (
    <span
      className={cx(styles["visor"], className)}
      data-state={state}
      data-size={size}
      data-tinted={tinted}
      {...(announce ? { role: "img", "aria-label": t(`visor.${state}`) } : { "aria-hidden": true })}
    >
      {glyph ? (
        <>
          <Glyph state={glyph} />
          <Glyph state={glyph} />
        </>
      ) : (
        <>
          <i className={styles["eye"]} />
          <i className={styles["eye"]} />
        </>
      )}
    </span>
  );
}
