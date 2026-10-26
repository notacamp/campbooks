import type { PropertyType } from "./types";

/** The small mark each type of column carries in a header and a menu. */
const GLYPHS: Record<PropertyType, string> = {
  title: "Aa",
  text: "Aa",
  number: "#",
  select: "◎",
  multi_select: "◉",
  status: "◐",
  date: "▦",
  checkbox: "✓",
  person: "@",
  files: "▢",
  url: "⧉",
  email: "✉",
  phone: "☎",
  relation: "⇄",
  campbooks_link: "↗",
  created: "⌁",
  edited: "⌁",
  unique_id: "ID",
  notion_formula: "ƒ",
  notion_rollup: "Σ",
  notion_people: "@",
  external: "·",
};

export function glyphOf(type: PropertyType): string {
  return GLYPHS[type];
}
