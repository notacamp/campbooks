/**
 * primitives.ts — barrel for the four Campbooks presentational primitives.
 *
 * The scaffold owner should add one line to index.ts:
 *   export * from "./primitives";
 *
 * These components are owned by the design session (campbooks-b4).
 */

export { Visor, type VisorProps, type VisorState } from "./visor/visor";

export {
  Signal,
  Dot,
  Label,
  Pill,
  type DotProps,
  type LabelProps,
  type PillProps,
  type Place,
} from "./signal/signal";

export {
  StatusGlyph,
  type StatusGlyphProps,
  type GlyphStatus,
} from "./status-glyph/status-glyph";

export { Avatar, type AvatarProps } from "./avatar/avatar";
