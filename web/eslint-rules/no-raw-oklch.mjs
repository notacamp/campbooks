/**
 * no-raw-oklch — HARD FAIL on raw oklch() color values in TypeScript source.
 *
 * "Raw" means `oklch(` immediately followed by a digit or `%` — i.e. a
 * literal color value, not a variable reference.
 *
 * NOT flagged:
 *   - `color-mix(in oklch, var(--cb-a) 30%, ...)` — "in oklch" is fine
 *   - `oklch(var(--cb-accent))` — variable reference, not a raw value
 *   - bare `in oklch` usage
 *
 * EXEMPT path: web/src/lib/ui/visor/ — the brand mark needs
 * theme-independent dark-glass colors that don't map to tokens.
 */

const OKLCH_RAW_RE = /oklch\(\s*[\d.%]/;

export default {
  meta: {
    type: "problem",
    docs: {
      description:
        "Ban raw oklch() color literals; use CSS variable tokens (--cb-*) instead.",
    },
    schema: [],
    messages: {
      forbidden:
        "Raw oklch() color is forbidden. Use a CSS token (e.g. var(--cb-accent)) instead.",
    },
  },

  create(context) {
    // Exemption: visor files may use theme-independent raw oklch values.
    const filename = context.filename ?? context.getFilename?.() ?? "";
    if (filename.includes("lib/ui/visor/")) return {};

    const check = (node, value) => {
      if (OKLCH_RAW_RE.test(value)) {
        context.report({ node, messageId: "forbidden" });
      }
    };

    return {
      Literal(node) {
        if (typeof node.value === "string") {
          check(node, node.value);
        }
      },
      TemplateElement(node) {
        const val = node.value?.cooked ?? node.value?.raw ?? "";
        if (val) check(node, val);
      },
    };
  },
};
