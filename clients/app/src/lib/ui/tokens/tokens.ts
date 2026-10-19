/**
 * Names of the tokens components may read, for typed use from TypeScript
 * (`cssVar("surface-raised")` → `var(--surface-raised)`). The values live in
 * tokens.css; `tokens.test.ts` fails if a name here is not declared there.
 *
 * Only layers 2 (semantic) and 3 (component) are listed: primitives are the
 * raw material for themes, never read by components.
 */
export const SEMANTIC_TOKENS = {
  surface: [
    "surface-base",
    "surface-raised",
    "surface-sunken",
    "surface-overlay",
    "surface-hover",
    "surface-selected",
    "surface-scrim",
  ],
  border: ["border-subtle", "border-strong", "edge-highlight"],
  scrollbar: ["scrollbar-thumb", "scrollbar-thumb-hover"],
  text: ["text-primary", "text-secondary", "text-tertiary", "text-disabled"],
  accent: [
    "accent",
    "accent-hover",
    "on-accent",
    "accent-soft",
    "accent-text",
    "highlight",
    "focus-ring",
  ],
  status: ["danger", "danger-soft", "success", "success-soft"],
  signal: [
    "signal-reply",
    "signal-decide",
    "signal-do",
    "signal-pay",
    "signal-review",
    "signal-sort",
    "signal-look",
    "signal-nudge",
  ],
  visor: [
    "visor-glass-top",
    "visor-glass-bottom",
    "visor-eye",
    "visor-eye-glow",
    "visor-rim",
    "visor-tint-star",
    "visor-tint-heart",
  ],
  elevation: ["shadow-color", "shadow-overlay"],
} as const;

export const COMPONENT_TOKENS = [
  "btn-height",
  "btn-radius",
  "btn-pad-x",
  "row-pad-y",
  "row-pad-x",
  "row-radius",
  "pill-height",
  "kbd-size",
  "kbd-bg",
  "kbd-fg",
  "kbd-border",
  "nav-height",
  "panel-radius",
  "overlay-radius",
  "visor-size",
  "sidebar-width",
  "aside-width",
  "list-width",
  "content-max",
  "toast-height",
  "overlay-width",
  "dialog-width",
  "clamp-height",
  "clamp-fade-height",
  "clamp-fade-to",
  "dialog-result-width",
  "popover-width",
  "offer-width",
  "offer-max-height",
  "progress-height",
  "narrow-width",
  "row-text-min",
  "scrollbar-size",
  "scrollbar-gap",
  "message-yours-indent",
  "message-yours-bg",
  "reader-footer-max",
] as const;

export type SemanticToken = (typeof SEMANTIC_TOKENS)[keyof typeof SEMANTIC_TOKENS][number];
export type ComponentToken = (typeof COMPONENT_TOKENS)[number];
export type Token = SemanticToken | ComponentToken;

/** `var(--<name>)` for a token, typed so a typo fails to compile. */
export function cssVar(name: Token): string {
  return `var(--${name})`;
}

/** The workspace accent presets (data-accent). `business` is ink: no hue. */
/**
 * Colours for the mail frame, which cannot read the app's CSS variables: mail
 * is always shown on a white page, in every theme.
 */
export const MAIL_PAGE = {
  /** The "Show quoted text" pill's dashed border. */
  foldBorder: "#c9c7c2",
  /** The pill's label, at AA contrast on white. */
  foldText: "#5f5d59",
} as const;

export const ACCENTS = ["business", "personal", "forest"] as const;
export type Accent = (typeof ACCENTS)[number];

/** Theme modes (data-theme). `system` follows the OS through light-dark(). */
export const THEME_MODES = ["system", "light", "dark"] as const;
export type ThemeMode = (typeof THEME_MODES)[number];

/** Densities (data-density). */
export const DENSITIES = ["comfortable", "compact"] as const;
export type Density = (typeof DENSITIES)[number];

/** Signals: what a queue item asks of you. Colour carries meaning only. */
export const SIGNALS = ["review", "reply", "decide", "do", "pay", "nudge", "sort", "look"] as const;
export type Signal = (typeof SIGNALS)[number];
