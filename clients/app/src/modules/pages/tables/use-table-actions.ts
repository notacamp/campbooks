import { useQueryClient, type InfiniteData, type QueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useUndoToast } from "~/lib/ui";

import { tableKeys, useTableCalls } from "./api";
import type {
  AddedProperty,
  CellInput,
  NewView,
  PropertyType,
  PropertyView,
  RowView,
  OptionDraft,
  TableDetail,
  TableRows,
  TableViewView,
} from "./types";

type RowsData = InfiniteData<TableRows, string | undefined>;

const hasPages = (data: unknown): data is RowsData =>
  typeof data === "object" && data !== null && Array.isArray((data as RowsData).pages);

/** Apply `change` to every cached copy of a table's rows (every view, and the by-id reads). */
function mapRows(
  client: QueryClient,
  workspaceId: string,
  tableId: string,
  change: (row: RowView) => RowView,
): void {
  client.setQueriesData<RowsData | TableRows>(
    { queryKey: tableKeys.rows(workspaceId, tableId) },
    (data) => {
      if (hasPages(data)) {
        return {
          ...data,
          pages: data.pages.map((page) => ({ ...page, rows: page.rows.map(change) })),
        };
      }
      return data === undefined ? data : { ...data, rows: data.rows.map(change) };
    },
  );
}

/** Put a row, as the server has it, in place of the one the cache holds. */
function replaceRow(client: QueryClient, workspaceId: string, tableId: string, fresh: RowView) {
  mapRows(client, workspaceId, tableId, (row) => (row.id === fresh.id ? fresh : row));
}

/** Add a row at the end of the last loaded page of every view (when the end is loaded). */
function appendRow(client: QueryClient, workspaceId: string, tableId: string, fresh: RowView) {
  client.setQueriesData<RowsData | TableRows>(
    { queryKey: tableKeys.rows(workspaceId, tableId) },
    (data) => {
      if (
        !hasPages(data) ||
        data.pages.some((page) => page.rows.some((row) => row.id === fresh.id))
      ) {
        return data;
      }
      const lastPage = data.pages.at(-1);
      if (
        lastPage === undefined ||
        (lastPage.next_cursor !== null && lastPage.next_cursor !== undefined)
      ) {
        return data;
      }
      return {
        ...data,
        pages: [
          ...data.pages.slice(0, -1),
          { ...lastPage, rows: [...lastPage.rows, fresh], matching: lastPage.matching + 1 },
        ],
      };
    },
  );
}

function dropRow(client: QueryClient, workspaceId: string, tableId: string, rowId: string) {
  client.setQueriesData<RowsData | TableRows>(
    { queryKey: tableKeys.rows(workspaceId, tableId) },
    (data) => {
      if (!hasPages(data)) {
        return data;
      }
      return {
        ...data,
        pages: data.pages.map((page) => ({
          ...page,
          rows: page.rows.filter((row) => row.id !== rowId),
          matching: Math.max(
            0,
            page.matching - (page.rows.some((row) => row.id === rowId) ? 1 : 0),
          ),
        })),
      };
    },
  );
}

/**
 * A member's changes to a table: cells, rows, columns, options and views.
 * Cell writes are optimistic and carry the version the member saw; a cell
 * somebody else changed meanwhile is said in the toast, and every write has
 * an Undo (and Redo). A failure rolls the cell back and stays until Retry.
 * Nothing in a toast names a value: only the property and the row's kind.
 */
export function useTableActions(workspaceId: string, tableId: string) {
  const { t } = useTranslation("pages");
  const client = useQueryClient();
  const calls = useTableCalls(workspaceId);
  const undo = useUndoToast();

  const refreshSchema = () => {
    void client.invalidateQueries({ queryKey: tableKeys.table(workspaceId, tableId) });
    void client.invalidateQueries({ queryKey: tableKeys.rows(workspaceId, tableId) });
  };

  /** Run `work`; on failure say so and offer to run it again. */
  const attempt = async <TResult>(
    label: string,
    work: () => Promise<TResult>,
    rollback?: () => void,
  ): Promise<TResult | undefined> => {
    try {
      return await work();
    } catch {
      rollback?.();
      undo.fail({
        label,
        retry: () => {
          void attempt(label, work, rollback);
        },
      });
      return undefined;
    }
  };

  // --------------------------------------------------------------------- cells

  const setCell = async (row: RowView, property: PropertyView, value: unknown): Promise<void> => {
    const before = row.cells[property.id];
    const label = t("tables.toast.cellFailed", { property: property.name });
    const cleared = value === null || value === undefined;
    mapRows(client, workspaceId, tableId, (candidate) => {
      if (candidate.id !== row.id) {
        return candidate;
      }
      const others = Object.fromEntries(
        Object.entries(candidate.cells).filter(([id]) => id !== property.id),
      );
      return {
        ...candidate,
        cells: cleared
          ? others
          : { ...others, [property.id]: { value, version: before?.version ?? 0 } },
      };
    });
    const input: CellInput = {
      property: property.id,
      value: value ?? null,
      ...(before === undefined ? {} : { version: before.version }),
    };
    const written = await attempt(
      label,
      () => calls.setCells(tableId, row.id, [input]),
      () => {
        replaceRow(client, workspaceId, tableId, row);
      },
    );
    if (written === undefined) {
      return;
    }
    replaceRow(client, workspaceId, tableId, written.row);
    const result = written.results[0];
    if (result === undefined) {
      return;
    }
    let version = result.version;
    const write = async (next: unknown) => {
      const done = await attempt(label, () =>
        calls.setCells(tableId, row.id, [{ property: property.id, value: next ?? null, version }]),
      );
      if (done !== undefined) {
        replaceRow(client, workspaceId, tableId, done.row);
        version = done.results[0]?.version ?? version;
      }
    };
    undo.push({
      label: result.stale
        ? t("tables.toast.cellStale", { property: property.name })
        : t("tables.toast.cell", { property: property.name }),
      undo: () => {
        void write(result.previous);
      },
      redo: () => {
        void write(value);
      },
    });
  };

  const setTitle = async (row: RowView, title: string): Promise<void> => {
    const label = t("tables.toast.titleFailed");
    mapRows(client, workspaceId, tableId, (candidate) =>
      candidate.id === row.id ? { ...candidate, title } : candidate,
    );
    const renamed = await attempt(
      label,
      () => calls.renameRow(row.id, row.meta_revision, title),
      () => {
        replaceRow(client, workspaceId, tableId, row);
      },
    );
    if (renamed === undefined) {
      return;
    }
    let meta = renamed.meta_revision;
    mapRows(client, workspaceId, tableId, (candidate) =>
      candidate.id === row.id
        ? { ...candidate, title: renamed.title, meta_revision: meta }
        : candidate,
    );
    const rename = async (next: string) => {
      const done = await attempt(label, () => calls.renameRow(row.id, meta, next));
      if (done !== undefined) {
        meta = done.meta_revision;
        mapRows(client, workspaceId, tableId, (candidate) =>
          candidate.id === row.id
            ? { ...candidate, title: done.title, meta_revision: meta }
            : candidate,
        );
      }
    };
    undo.push({
      label: t("tables.toast.title"),
      undo: () => {
        void rename(row.title);
      },
      redo: () => {
        void rename(title);
      },
    });
  };

  // ---------------------------------------------------------------------- rows

  const addRow = async (
    init: { title?: string; cells?: CellInput[] } = {},
  ): Promise<RowView | undefined> => {
    const made = await attempt(t("tables.toast.rowFailed"), () => calls.addRow(tableId, init));
    if (made === undefined) {
      return undefined;
    }
    appendRow(client, workspaceId, tableId, made);
    void client.invalidateQueries({ queryKey: tableKeys.table(workspaceId, tableId) });
    undo.push({
      label: t("tables.toast.row"),
      undo: () => {
        void attempt(t("tables.toast.rowUndoFailed"), async () => {
          await calls.trashPage(made.id);
          dropRow(client, workspaceId, tableId, made.id);
          void client.invalidateQueries({ queryKey: tableKeys.table(workspaceId, tableId) });
        });
      },
      redo: () => {
        void attempt(t("tables.toast.rowUndoFailed"), async () => {
          await calls.restorePage(made.id);
          refreshSchema();
        });
      },
    });
    return made;
  };

  const trashRow = async (row: RowView): Promise<boolean> => {
    dropRow(client, workspaceId, tableId, row.id);
    const done = await attempt(
      t("tables.toast.trashFailed"),
      async () => {
        await calls.trashPage(row.id);
        return true;
      },
      refreshSchema,
    );
    if (done !== true) {
      return false;
    }
    refreshSchema();
    undo.push({
      label: t("tables.toast.trashed"),
      undo: () => {
        void attempt(t("tables.toast.rowUndoFailed"), async () => {
          await calls.restorePage(row.id);
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.trashFailed"), async () => {
          await calls.trashPage(row.id);
          refreshSchema();
        });
      },
    });
    return true;
  };

  // ------------------------------------------------------------------ columns

  const addProperty = async (
    body: Parameters<typeof calls.addProperty>[1],
  ): Promise<AddedProperty | undefined> => {
    const added = await attempt(t("tables.toast.columnFailed"), () =>
      calls.addProperty(tableId, body),
    );
    if (added === undefined) {
      return undefined;
    }
    refreshSchema();
    const id = added.property.id;
    undo.push({
      label: t("tables.toast.column", { property: added.property.name }),
      undo: () => {
        void attempt(t("tables.toast.columnUndoFailed"), async () => {
          await calls.removeProperty(tableId, id);
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.columnUndoFailed"), async () => {
          await calls.restoreProperty(tableId, id);
          refreshSchema();
        });
      },
    });
    return added;
  };

  const renameProperty = async (property: PropertyView, name: string): Promise<void> => {
    const done = await attempt(t("tables.toast.columnFailed"), () =>
      calls.updateProperty(tableId, property.id, { name }),
    );
    if (done === undefined) {
      return;
    }
    refreshSchema();
    const rename = async (next: string) => {
      await attempt(t("tables.toast.columnFailed"), async () => {
        await calls.updateProperty(tableId, property.id, { name: next });
        refreshSchema();
      });
    };
    undo.push({
      label: t("tables.toast.columnRenamed", { property: name }),
      undo: () => {
        void rename(property.name);
      },
      redo: () => {
        void rename(name);
      },
    });
  };

  const changeType = async (
    property: PropertyView,
    type: PropertyType,
    config?: unknown,
  ): Promise<void> => {
    const done = await attempt(t("tables.toast.columnFailed"), () =>
      calls.updateProperty(tableId, property.id, {
        type,
        ...(config === undefined ? {} : { config }),
      }),
    );
    if (done === undefined) {
      return;
    }
    refreshSchema();
    undo.push({
      label: t("tables.toast.columnType", { property: property.name }),
      undo: () => {
        void attempt(t("tables.toast.columnUndoFailed"), async () => {
          await (done.can_revert
            ? calls.restoreProperty(tableId, property.id)
            : calls.updateProperty(tableId, property.id, {
                type: property.type,
                config: property.config,
              }));
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.columnFailed"), async () => {
          await calls.updateProperty(tableId, property.id, {
            type,
            ...(config === undefined ? {} : { config }),
          });
          refreshSchema();
        });
      },
    });
  };

  const updateConfig = async (property: PropertyView, config: unknown): Promise<void> => {
    const done = await attempt(t("tables.toast.columnFailed"), () =>
      calls.updateProperty(tableId, property.id, { config }),
    );
    if (done === undefined) {
      return;
    }
    refreshSchema();
    undo.push({
      label: t("tables.toast.columnChanged", { property: property.name }),
      undo: () => {
        void attempt(t("tables.toast.columnUndoFailed"), async () => {
          await calls.updateProperty(tableId, property.id, { config: property.config });
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.columnFailed"), async () => {
          await calls.updateProperty(tableId, property.id, { config });
          refreshSchema();
        });
      },
    });
  };

  /**
   * Remove a column. For a table kept in step with Notion `confirm` says the
   * member confirmed removing it there too; `probe` tries without it first
   * (a column that may be only Campbooks') and answers `confirm` when the
   * server says it is Notion's too, changing nothing.
   */
  const removeProperty = async (
    property: PropertyView,
    options: { confirm?: boolean; probe?: boolean } = {},
  ): Promise<"removed" | "failed" | "confirm"> => {
    const confirm = options.confirm === true;
    const run = () => calls.removeProperty(tableId, property.id, confirm);
    let done;
    if (options.probe === true) {
      try {
        done = await run();
      } catch (error) {
        if (problemCode(error) === "confirmation_required") {
          return "confirm";
        }
        undo.fail({
          label: t("tables.toast.columnFailed"),
          retry: () => {
            void removeProperty(property, options);
          },
        });
        return "failed";
      }
    } else {
      done = await attempt(t("tables.toast.columnFailed"), run);
    }
    if (done === undefined) {
      return "failed";
    }
    refreshSchema();
    undo.push({
      label: t(confirm ? "tables.toast.columnRemovedBoth" : "tables.toast.columnRemoved", {
        property: property.name,
      }),
      undo: () => {
        void attempt(t("tables.toast.columnUndoFailed"), async () => {
          await calls.restoreProperty(tableId, property.id);
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.columnFailed"), async () => {
          await run();
          refreshSchema();
        });
      },
    });
    return "removed";
  };

  /** Add, rename, recolour, reorder or remove options (Undo sends the old list back, ids and all). */
  const setOptions = async (
    property: PropertyView,
    options: readonly OptionDraft[],
    previous: readonly OptionDraft[],
  ): Promise<PropertyView | undefined> => {
    const done = await attempt(t("tables.toast.optionsFailed"), () =>
      calls.setOptions(tableId, property.id, options),
    );
    if (done === undefined) {
      return undefined;
    }
    refreshSchema();
    undo.push({
      label: t("tables.toast.options", { property: property.name }),
      undo: () => {
        void attempt(t("tables.toast.optionsFailed"), async () => {
          await calls.setOptions(tableId, property.id, previous);
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.optionsFailed"), async () => {
          await calls.setOptions(tableId, property.id, options);
          refreshSchema();
        });
      },
    });
    return done;
  };

  // -------------------------------------------------------------------- views

  /** Patch a view in the cached table at once, so the screen answers before the server does. */
  const patchView = (viewId: string, change: (view: TableViewView) => TableViewView) => {
    client.setQueryData<TableDetail>(tableKeys.table(workspaceId, tableId), (data) =>
      data === undefined
        ? data
        : { ...data, views: data.views.map((view) => (view.id === viewId ? change(view) : view)) },
    );
  };

  const updateView = async (
    view: TableViewView,
    change: { name?: string; config?: unknown },
    label = t("tables.toast.view", { view: view.name }),
  ): Promise<void> => {
    patchView(view.id, (current) => ({
      ...current,
      ...(change.name === undefined ? {} : { name: change.name }),
      ...(change.config === undefined ? {} : { config: change.config }),
    }));
    const done = await attempt(
      t("tables.toast.viewFailed"),
      () => calls.updateView(tableId, view.id, change),
      refreshSchema,
    );
    if (done === undefined) {
      return;
    }
    refreshSchema();
    undo.push({
      label,
      undo: () => {
        patchView(view.id, () => view);
        void attempt(t("tables.toast.viewFailed"), async () => {
          await calls.updateView(tableId, view.id, { name: view.name, config: view.config });
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.viewFailed"), async () => {
          await calls.updateView(tableId, view.id, change);
          refreshSchema();
        });
      },
    });
  };

  const createView = async (body: NewView): Promise<TableViewView | undefined> => {
    const made = await attempt(t("tables.toast.viewFailed"), () => calls.createView(tableId, body));
    if (made === undefined) {
      return undefined;
    }
    refreshSchema();
    undo.push({
      label: t("tables.toast.viewAdded", { view: made.name }),
      undo: () => {
        void attempt(t("tables.toast.viewFailed"), async () => {
          await calls.removeView(tableId, made.id);
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.viewFailed"), async () => {
          await calls.createView(tableId, {
            id: made.id,
            name: made.name,
            type: made.type,
            position: made.position,
            config: made.config,
          });
          refreshSchema();
        });
      },
    });
    return made;
  };

  const removeView = async (view: TableViewView): Promise<boolean> => {
    const done = await attempt(t("tables.toast.viewFailed"), () =>
      calls.removeView(tableId, view.id),
    );
    if (done === undefined) {
      return false;
    }
    refreshSchema();
    undo.push({
      label: t("tables.toast.viewRemoved", { view: view.name }),
      undo: () => {
        void attempt(t("tables.toast.viewFailed"), async () => {
          await calls.createView(tableId, {
            id: view.id,
            name: view.name,
            type: view.type,
            position: view.position,
            config: view.config,
          });
          refreshSchema();
        });
      },
      redo: () => {
        void attempt(t("tables.toast.viewFailed"), async () => {
          await calls.removeView(tableId, view.id);
          refreshSchema();
        });
      },
    });
    return true;
  };

  return {
    calls,
    setCell,
    setTitle,
    addRow,
    trashRow,
    addProperty,
    renameProperty,
    changeType,
    updateConfig,
    removeProperty,
    setOptions,
    updateView,
    createView,
    removeView,
    refresh: refreshSchema,
  };
}

export type TableActions = ReturnType<typeof useTableActions>;
