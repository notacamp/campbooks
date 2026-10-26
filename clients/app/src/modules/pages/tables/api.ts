import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { api, errorStatus, operationKey, unwrap } from "~/lib/api";

import type {
  AddedProperty,
  CellInput,
  CellsWritten,
  NewProperty,
  NewView,
  PropertyView,
  RowView,
  OptionDraft,
  TableDetail,
  TableQueryBody,
  TableRows,
  TableViewView,
} from "./types";

// Keyed by operationId, so the change feed refetches them when a
// `pages.page` (a table, or one of its rows) changes.
const GET_TABLE = "pages.get_table";
const QUERY_TABLE = "pages.query_table";

/** The page of rows the server answers at most. */
const ROWS_PER_PAGE = 100;

export const tableKeys = {
  table: (workspaceId: string, tableId: string) =>
    operationKey(GET_TABLE, { workspace_id: workspaceId, table_id: tableId }),
  /** Every rows query of one table (a prefix: views and extras share it). */
  rows: (workspaceId: string, tableId: string) =>
    operationKey(QUERY_TABLE, { workspace_id: workspaceId, table_id: tableId }),
  view: (workspaceId: string, tableId: string, viewId: string) =>
    operationKey(QUERY_TABLE, { workspace_id: workspaceId, table_id: tableId, view: viewId }),
};

/** A table the server answers 404 to is one the caller cannot open (or tables are off). */
const retryUnlessMissing = (count: number, error: unknown) =>
  errorStatus(error) !== 404 && count < 2;

/** `pages.get_table`: the schema (properties and views) of a table. */
export function useTable(workspaceId: string, tableId: string) {
  return useQuery({
    queryKey: tableKeys.table(workspaceId, tableId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/tables/{table_id}", {
          params: { path: { workspace_id: workspaceId, table_id: tableId } },
          signal,
        }),
      ),
    retry: retryUnlessMissing,
  });
}

/** `pages.query_table` for a view, 100 rows at a time (the cursor is the server's). */
export function useTableRows(workspaceId: string, tableId: string, viewId?: string) {
  return useInfiniteQuery({
    queryKey: tableKeys.view(workspaceId, tableId, viewId ?? ""),
    enabled: viewId !== undefined,
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ signal, pageParam }): Promise<TableRows> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/query", {
          params: { path: { workspace_id: workspaceId, table_id: tableId } },
          body: {
            ...(viewId === undefined ? {} : { view: viewId }),
            ...(pageParam === undefined ? {} : { cursor: pageParam }),
            limit: ROWS_PER_PAGE,
          },
          signal,
        }),
      ),
    getNextPageParam: (last) => last.next_cursor ?? undefined,
    retry: retryUnlessMissing,
  });
}

/** Rows of a table, by id: what a relation cell names. */
export function useRowsById(
  workspaceId: string,
  tableId: string | undefined,
  ids: readonly string[],
) {
  const wanted = [...new Set(ids)].toSorted().slice(0, ROWS_PER_PAGE);
  return useQuery({
    queryKey: operationKey(QUERY_TABLE, {
      workspace_id: workspaceId,
      table_id: tableId ?? "",
      rows: wanted.join(","),
    }),
    enabled: tableId !== undefined && wanted.length > 0,
    queryFn: async ({ signal }): Promise<TableRows> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/query", {
          params: { path: { workspace_id: workspaceId, table_id: tableId ?? "" } },
          body: { rows: wanted, limit: ROWS_PER_PAGE },
          signal,
        }),
      ),
    retry: retryUnlessMissing,
    staleTime: 30_000,
  });
}

/** The calls a table takes. Each answers what the server made of it. */
export function useTableCalls(workspaceId: string) {
  return useMemo(() => tableCalls(workspaceId), [workspaceId]);
}

function tableCalls(workspaceId: string) {
  const path = { workspace_id: workspaceId };
  const at = (table: string) => ({ path: { ...path, table_id: table } });
  const propertyAt = (table: string, property: string) => ({
    path: { ...path, table_id: table, property_id: property },
  });
  const viewAt = (table: string, view: string) => ({
    path: { ...path, table_id: table, view_id: view },
  });
  const pageAt = (page: string) => ({ path: { ...path, page_id: page } });
  return {
    createTable: async (body: {
      title?: string;
      parent?: string | null;
      place?: "private" | "workspace";
      title_property?: string;
      view_name?: string;
    }): Promise<TableDetail> =>
      unwrap(await api.POST("/api/v1/w/{workspace_id}/tables", { params: { path }, body })),
    getTable: async (table: string): Promise<TableDetail> =>
      unwrap(await api.GET("/api/v1/w/{workspace_id}/tables/{table_id}", { params: at(table) })),
    query: async (table: string, body: TableQueryBody, signal?: AbortSignal): Promise<TableRows> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/query", {
          params: at(table),
          body,
          ...(signal === undefined ? {} : { signal }),
        }),
      ),
    addProperty: async (table: string, body: NewProperty): Promise<AddedProperty> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/properties", {
          params: at(table),
          body,
        }),
      ),
    updateProperty: async (
      table: string,
      property: string,
      body: { name?: string; type?: PropertyView["type"]; config?: unknown },
    ): Promise<PropertyView> =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}", {
          params: propertyAt(table, property),
          body,
        }),
      ),
    removeProperty: async (table: string, property: string): Promise<PropertyView> =>
      unwrap(
        await api.DELETE("/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}", {
          params: propertyAt(table, property),
        }),
      ),
    restoreProperty: async (table: string, property: string): Promise<PropertyView> =>
      unwrap(
        await api.POST(
          "/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}/restore",
          { params: propertyAt(table, property) },
        ),
      ),
    setOptions: async (
      table: string,
      property: string,
      options: readonly OptionDraft[],
    ): Promise<PropertyView> =>
      unwrap(
        await api.PUT(
          "/api/v1/w/{workspace_id}/tables/{table_id}/properties/{property_id}/options",
          { params: propertyAt(table, property), body: { options: [...options] } },
        ),
      ),
    addRow: async (
      table: string,
      body: { title?: string; cells?: CellInput[] },
    ): Promise<RowView> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/rows", {
          params: at(table),
          body,
        }),
      ),
    setCells: async (table: string, row: string, cells: CellInput[]): Promise<CellsWritten> =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/tables/{table_id}/rows/{row_id}/cells", {
          params: { path: { ...path, table_id: table, row_id: row } },
          body: { cells },
        }),
      ),
    createView: async (table: string, body: NewView): Promise<TableViewView> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/tables/{table_id}/views", {
          params: at(table),
          body,
        }),
      ),
    updateView: async (
      table: string,
      view: string,
      body: { name?: string; config?: unknown },
    ): Promise<TableViewView> =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/tables/{table_id}/views/{view_id}", {
          params: viewAt(table, view),
          body,
        }),
      ),
    removeView: async (table: string, view: string): Promise<TableViewView> =>
      unwrap(
        await api.DELETE("/api/v1/w/{workspace_id}/tables/{table_id}/views/{view_id}", {
          params: viewAt(table, view),
        }),
      ),
    /** A row's title, which is its page's. */
    renameRow: async (row: string, metaRevision: number, title: string) =>
      unwrap(
        await api.PATCH("/api/v1/w/{workspace_id}/pages/{page_id}", {
          params: pageAt(row),
          body: { meta_revision: metaRevision, title },
        }),
      ),
    trashPage: async (row: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/trash", { params: pageAt(row) }),
      ),
    restorePage: async (row: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/restore", { params: pageAt(row) }),
      ),
  };
}
