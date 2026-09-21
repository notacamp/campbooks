/**
 * Visor — Scout's face and the Campbooks brand mark.
 *
 * A black-glass visor with two white eyes. Theme-independent — always dark
 * glass regardless of the app's light/dark setting.
 *
 * The `gaze` prop enables pointer-follow: the eyes track the cursor within
 * a 30% drift relative to the visor size. Disabled automatically when
 * prefers-reduced-motion is set.
 */
import {
  forwardRef,
  useEffect,
  useRef,
  useCallback,
  type HTMLAttributes,
  type CSSProperties,
} from "react";
import { cn } from "~/lib/utils";
import styles from "./visor.module.css";

// ── Types ──────────────────────────────────────────────────────────────────────

export type VisorState =
  | "watching"
  | "reading"
  | "thinking"
  | "found"
  | "happy"
  | "asleep";

export interface VisorProps extends HTMLAttributes<HTMLSpanElement> {
  /** Animated eye state. Defaults to "watching" (idle blink). */
  state?: VisorState;
  /** Size in px — controls width and proportional height. Default 24. */
  size?: number;
  /**
   * When true, eyes drift toward the pointer (max ≈ 30% of size).
   * Automatically suppressed under prefers-reduced-motion.
   */
  gaze?: boolean;
  /**
   * Accessible label. When provided, role="img" + aria-label are set.
   * Omitting it leaves aria-hidden="true" (decorative).
   */
  label?: string;
}

// ── Component ──────────────────────────────────────────────────────────────────

/**
 * Inline SVG-like brand mark rendered with pure CSS. Animates the eyes
 * according to `state`.
 */
export const Visor = forwardRef<HTMLSpanElement, VisorProps>(
  (
    {
      state = "watching",
      size = 24,
      gaze = false,
      label,
      className,
      style,
      ...props
    },
    ref,
  ) => {
    const containerRef = useRef<HTMLSpanElement | null>(null);
    const eye1Ref = useRef<HTMLSpanElement | null>(null);
    const eye2Ref = useRef<HTMLSpanElement | null>(null);

    // Merge forwarded ref with local ref
    const setRef = useCallback(
      (node: HTMLSpanElement | null) => {
        containerRef.current = node;
        if (typeof ref === "function") {
          ref(node);
        } else if (ref) {
          ref.current = node;
        }
      },
      [ref],
    );

    // Pointer-gaze effect
    useEffect(() => {
      if (!gaze) return;

      const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
      if (mq.matches) return;

      const maxDrift = size * 0.3;

      const onPointerMove = (e: PointerEvent): void => {
        const container = containerRef.current;
        if (!container) return;
        const rect = container.getBoundingClientRect();
        const cx = rect.left + rect.width / 2;
        const cy = rect.top + rect.height / 2;
        const dx = ((e.clientX - cx) / window.innerWidth) * maxDrift;
        const dy = ((e.clientY - cy) / window.innerHeight) * maxDrift;
        const clamp = (v: number, limit: number): number =>
          Math.max(-limit, Math.min(limit, v));
        const gx = `${clamp(dx, maxDrift)}px`;
        const gy = `${clamp(dy, maxDrift * 0.5)}px`;
        [eye1Ref.current, eye2Ref.current].forEach((eye) => {
          if (eye) {
            eye.style.setProperty("--gx", gx);
            eye.style.setProperty("--gy", gy);
          }
        });
      };

      window.addEventListener("pointermove", onPointerMove, { passive: true });
      return () => {
        window.removeEventListener("pointermove", onPointerMove);
      };
    }, [gaze, size]);

    const stateClass: string | undefined =
      state !== "watching" ? styles[state] : undefined;

    const cssVars = {
      "--visor-s": `${size}px`,
      ...style,
    } as CSSProperties;

    const a11yProps = label
      ? { role: "img" as const, "aria-label": label }
      : { "aria-hidden": true as const };

    return (
      <span
        ref={setRef}
        className={cn(styles.visor, stateClass, className)}
        style={cssVars}
        {...a11yProps}
        {...props}
      >
        <span ref={eye1Ref} className={styles.eye} />
        <span ref={eye2Ref} className={styles.eye} />
      </span>
    );
  },
);

Visor.displayName = "Visor";
