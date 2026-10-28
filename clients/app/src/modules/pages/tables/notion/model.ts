import type { Schema } from "~/lib/api";

import type { PropertyType, PropertyView } from "../types";

export type TableSync = Schema<"NotionTableSync">;
export type TableConflict = Schema<"TableConflict">;
export type TableConflictKeep = Schema<"TableConflictKeep">;

/** Why a Notion column cannot be edited here: what Notion computes or keeps. */
export type Lock = "formula" | "rollup" | "people" | "external";

/** The reserved Notion types, by the reason they are read-only. */
export function lockOf(type: PropertyType): Lock | undefined {
  switch (type) {
    case "notion_formula": {
      return "formula";
    }
    case "notion_rollup": {
      return "rollup";
    }
    case "notion_people": {
      return "people";
    }
    case "external": {
      return "external";
    }
    default: {
      return undefined;
    }
  }
}

/** The types Notion can make a column of: what the sync sends when one is made here. */
const NOTION_MAKES = new Set<PropertyType>([
  "title",
  "text",
  "number",
  "select",
  "multi_select",
  "date",
  "checkbox",
  "url",
  "email",
  "phone",
]);

/**
 * Whether a column of this type is sent to Notion at all. A status column made
 * here, a person of the workspace, files, a relation, a Campbooks link and the
 * created, edited and ID columns stay in Campbooks.
 */
export function reachesNotion(type: PropertyType): boolean {
  return NOTION_MAKES.has(type) || lockOf(type) !== undefined;
}

/**
 * The columns that are Campbooks' own and never sent: every type Notion can
 * neither make nor own. A status column is not among them (Notion has its
 * own), but one made here stays here; see {@link staysInCampbooks}.
 */
export function campbooksOnly(type: PropertyType): boolean {
  return !reachesNotion(type) && type !== "status";
}

/** Whether making or changing a column to this type stays in Campbooks. */
export function staysInCampbooks(type: PropertyType): boolean {
  return !reachesNotion(type);
}

/** What a column of a synced table says about itself, if anything. */
export type Mark = { kind: "locked"; lock: Lock } | { kind: "campbooks_only" } | { kind: "none" };

export function markOf(property: PropertyView): Mark {
  const lock = lockOf(property.type);
  if (lock !== undefined) {
    return { kind: "locked", lock };
  }
  return campbooksOnly(property.type) ? { kind: "campbooks_only" } : { kind: "none" };
}

/** The key a cell's conflict is found by. */
export const cellKey = (row: string, property: string) => `${row}:${property}`;

/** The cell conflicts waiting for a choice, by row and column. */
export function cellConflicts(conflicts: readonly TableConflict[]): Map<string, TableConflict> {
  return new Map(
    conflicts
      .filter((conflict) => conflict.kind === "cell" && typeof conflict.row === "string")
      .map((conflict) => [cellKey(conflict.row ?? "", conflict.property), conflict]),
  );
}

/** The column conflicts waiting for a choice, oldest first. */
export function columnConflicts(conflicts: readonly TableConflict[]): TableConflict[] {
  return conflicts.filter((conflict) => conflict.kind === "column");
}

/** A version of a column in a conflict: its name, type and how many options it has. */
export interface ColumnVersion {
  name: string;
  type: PropertyType | undefined;
  options: number;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** Read one side of a column conflict (`{name, type, config}`), whatever the server put there. */
export function columnVersion(raw: unknown): ColumnVersion {
  const version = isRecord(raw) ? raw : {};
  const config = isRecord(version["config"]) ? version["config"] : {};
  const options = config["options"];
  return {
    name: typeof version["name"] === "string" ? version["name"] : "",
    type: typeof version["type"] === "string" ? (version["type"] as PropertyType) : undefined,
    options: Array.isArray(options) ? options.length : 0,
  };
}

export type ColumnDifference = "name" | "type" | "options";

/** What differs between the two versions of a column. */
export function columnDifferences(ours: ColumnVersion, theirs: ColumnVersion): ColumnDifference[] {
  const found: ColumnDifference[] = [];
  if (ours.name !== theirs.name) {
    found.push("name");
  }
  if (ours.type !== theirs.type) {
    found.push("type");
  }
  if (ours.options !== theirs.options) {
    found.push("options");
  }
  return found;
}

/** Whether the table is kept in step now (a stopped link marks nothing). */
export function isSyncing(sync?: TableSync): boolean {
  return sync?.link !== undefined && sync.link !== null && sync.link.state !== "stopped";
}
