import { createContext, use } from "react";

import type { TableConflict, TableSync } from "./model";
import { cellConflicts } from "./model";
import type { TableSyncActions } from "./use-sync-actions";

/** What the table's screens know of its Notion sync (nothing, for an ordinary table). */
export interface SyncedTable {
  /** Kept in step now: Notion marks and "stays in Campbooks" notes apply. */
  syncing: boolean;
  /** The member may settle a conflict. */
  canResolve: boolean;
  /** The cell conflicts, by row and column. */
  cells: ReadonlyMap<string, TableConflict>;
  resolve: TableSyncActions["resolve"];
}

const NONE: SyncedTable = {
  syncing: false,
  canResolve: false,
  cells: new Map(),
  resolve: () => Promise.resolve(false),
};

export const SyncedTableContext = createContext<SyncedTable>(NONE);

/** The sync of the table around the caller. */
export function useSyncedTable(): SyncedTable {
  return use(SyncedTableContext);
}

/** Build the context's value from the sync read and the actions. */
export function syncedTable(
  sync: TableSync | undefined,
  syncing: boolean,
  canResolve: boolean,
  resolve: TableSyncActions["resolve"],
): SyncedTable {
  return {
    syncing,
    canResolve,
    cells: syncing ? cellConflicts(sync?.conflicts ?? []) : new Map(),
    resolve,
  };
}
