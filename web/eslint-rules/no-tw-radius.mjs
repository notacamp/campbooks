/**
 * no-tw-radius — WARN on non-token Tailwind radius utilities.
 *
 * Campbooks uses --cb-r1..4 (6/8/12/16px) via the rounded-cb-* utilities
 * defined in @theme inline in tokens.css. The Tailwind defaults (rounded-sm,
 * rounded-md, etc.) and shadcn's --radius variable are off-system.
 *
 * Allowed:   rounded-full, rounded-none, rounded-cb-*, rounded-[--cb-r*]
 * Warn on:   rounded-sm, rounded-md, rounded-lg, rounded-xl, rounded-2xl,
 *            rounded-3xl, rounded-[--radius], var(--radius)
 */

const RADIUS_CLASS_RE = /\brounded-(sm|md|lg|xl|2xl|3xl)\b/;
const RADIUS_VAR_RE = /\brounded-\[--radius]|\bvar\(--radius\)/;

export default {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Use token-based radius utilities (rounded-cb-1..4) instead of Tailwind or shadcn defaults.",
    },
    schema: [],
    messages: {
      radius:
        "Non-token radius '{{cls}}' — use rounded-cb-1, rounded-cb-2, rounded-cb-3, or rounded-cb-4 instead.",
      radiusVar:
        "shadcn default --radius is off-system — use var(--cb-r1), var(--cb-r2), var(--cb-r3), or var(--cb-r4) instead.",
    },
  },

  create(context) {
    const check = (node, value) => {
      const m1 = RADIUS_CLASS_RE.exec(value);
      if (m1) {
        context.report({ node, messageId: "radius", data: { cls: m1[0] } });
        return;
      }
      if (RADIUS_VAR_RE.test(value)) {
        context.report({ node, messageId: "radiusVar" });
      }
    };

    return {
      Literal(node) {
        if (typeof node.value === "string") check(node, node.value);
      },
      TemplateElement(node) {
        const val = node.value?.cooked ?? node.value?.raw ?? "";
        if (val) check(node, val);
      },
    };
  },
};
