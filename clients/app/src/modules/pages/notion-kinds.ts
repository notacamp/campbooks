/** The Notion block types a "Kept in Notion" box names in words. */
export const KEPT_KINDS = [
  "column_list",
  "synced_block",
  "table",
  "child_database",
  "bookmark",
  "embed",
  "link_preview",
  "equation",
  "video",
  "audio",
  "pdf",
  "table_of_contents",
  "breadcrumb",
  "template",
  "unsupported",
] as const;

export type KeptKind = (typeof KEPT_KINDS)[number];

/** A type this client has no word for: "some_new_block" reads as "Some new block". */
export function humanKind(kind: string): string {
  const words = kind.replaceAll(/[_-]+/gu, " ").trim();
  return words === "" ? "" : words.charAt(0).toLocaleUpperCase() + words.slice(1);
}
