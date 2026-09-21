/**
 * no-raw-hex — HARD FAIL on raw hex color literals in TypeScript source.
 *
 * All colors must flow from CSS custom properties declared in tokens.css
 * (--cb-*, --color-*, etc.). A raw hex in a .ts or .tsx file means something
 * slipped past the token system.
 *
 * Pattern: #[0-9A-Fa-f]{3,8} NOT followed by another hex digit.
 * This matches #RGB, #RGBA, #RRGGBB, #RRGGBBAA but NOT 9+ hex char
 * strings (e.g. IDs/hashes that happen to look like hex).
 */

const HEX_RE = /#[0-9A-Fa-f]{3,8}(?![0-9A-Fa-f])/;

export default {
  meta: {
    type: "problem",
    docs: {
      description:
        "Ban raw hex color literals; use CSS variable tokens (--cb-*) instead.",
    },
    schema: [],
    messages: {
      forbidden:
        "Raw hex color '{{hex}}' is forbidden. Use a CSS token (e.g. var(--cb-accent)) instead.",
    },
  },

  create(context) {
    const check = (node, value) => {
      const match = HEX_RE.exec(value);
      if (match) {
        context.report({
          node,
          messageId: "forbidden",
          data: { hex: match[0] },
        });
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
