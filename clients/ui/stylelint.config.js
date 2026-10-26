// @ts-check
// Token-only styling (AGENTS.md → UI). Outside src/tokens/ no CSS may
// contain a colour, size, duration or easing literal: read a token instead
// (`var(--surface-raised)`, `var(--space-3)`, `var(--duration-fast)`).
// Unitless numbers (multipliers, flex factors, 0 and 1) and %, fr and deg
// are allowed. Media queries may use the rem breakpoints from
// tailwind-theme.css (40, 48, 64 and 80rem): custom properties can't be used there.

const MESSAGE = "Use a design token from src/tokens/tokens.css instead of a literal.";

const ONLY_TOKENS = [String.raw`/^var\(/`, "inherit", "initial", "unset"];

/** @type {import("stylelint").Config} */
export default {
  reportNeedlessDisables: true,
  rules: {
    "color-no-hex": [true, { message: MESSAGE }],
    "color-named": ["never", { message: MESSAGE }],
    "function-disallowed-list": [
      [
        "rgb",
        "rgba",
        "hsl",
        "hsla",
        "hwb",
        "lab",
        "lch",
        "oklab",
        "oklch",
        "color",
        "light-dark",
        "cubic-bezier",
        "steps",
      ],
      { message: MESSAGE },
    ],
    "declaration-property-unit-allowed-list": [
      { "/.*/": ["%", "fr", "deg"] },
      { message: MESSAGE },
    ],
    "media-feature-name-unit-allowed-list": [{ width: ["rem"] }, { message: MESSAGE }],
    "declaration-property-value-allowed-list": [
      {
        "z-index": ONLY_TOKENS,
        "font-family": ONLY_TOKENS,
        "font-size": ONLY_TOKENS,
        "font-weight": ONLY_TOKENS,
        "letter-spacing": ONLY_TOKENS,
        "line-height": ONLY_TOKENS,
        opacity: [...ONLY_TOKENS, "0", "1"],
      },
      { message: MESSAGE },
    ],
    "custom-property-no-missing-var-function": true,
  },
  overrides: [
    {
      // The one place literals live.
      files: ["src/tokens/**/*.css"],
      rules: {
        "color-no-hex": null,
        "color-named": null,
        "function-disallowed-list": null,
        "declaration-property-unit-allowed-list": null,
        "declaration-property-value-allowed-list": null,
      },
    },
  ],
};
