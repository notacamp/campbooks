/**
 * lib/ui — the design system barrel.
 *
 * Every component lives in its own kebab-case subdirectory following the
 * sidecar pattern: `lib/ui/<name>/<name>.tsx` (+ `.test.tsx`/`.stories.tsx`),
 * with a PascalCase export — matches connect-web-v2 (button/, signal/, …).
 *
 * RULES (enforced by lint + code review):
 *   1. Raw HTML elements and Tailwind classes live ONLY in this package.
 *   2. Consumers (modules) pass directive props (variant, size, color, etc.)
 *      — never className or style props.
 *   3. lib/ui may import other lib/*, never src/modules/*.
 *
 * The design session (campbooks-b4) owns the visual contract. The scaffold
 * session (this file) owns the infrastructure (shadcn setup, CVA wiring,
 * Radix primitives). Add new components via `npx shadcn add <name>` then
 * skin via tokens.css variables.
 */

// ── shadcn base (scaffold-owned) ────────────────────────────────────────────
export { Button, type ButtonProps } from "./button/button";

// ── Campbooks primitives (design-session-owned) ─────────────────────────────
// Visor, Signal (+ Dot/Label/Pill), StatusGlyph, Avatar — see ./primitives.ts
export * from "./primitives";

// ── Skim overlay (design-session-owned) — for wiring into Today/Inbox later ──
export { Skim, type SkimCard, type SkimAction, type SkimProps } from "./skim/skim";
