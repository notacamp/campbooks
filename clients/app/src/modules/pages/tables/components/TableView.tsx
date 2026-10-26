import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Button,
  DocCard,
  EmptyState,
  FilterChip,
  MentionChip,
  Popover,
  SkeletonList,
  TableToolbar,
  ToolbarButton,
  TableTabs,
  type CellPos,
} from "~/lib/ui";
import { useShortcut } from "~/lib/shortcuts";
import { useMembers } from "~/modules/workspaces";

import type { usePageActions } from "../../components/use-page-actions";
import type { PageSummary } from "../../types";
import { memberName, type CellContext } from "../cells/context";
import { useTable, useTableRows } from "../api";
import {
  columnsOf,
  configJson,
  flatFilter,
  filterOf,
  idsOf,
  optionById,
  specsOf,
  viewConfig,
} from "../model";
import { useTableActions } from "../use-table-actions";
import type {
  Condition,
  FilterNode,
  NewProperty,
  PropertyType,
  PropertyView,
  RowView,
  TableDetail,
  TableViewType,
} from "../types";

import { FilterBuilder, SortBuilder } from "./Builders";
import { BoardPane } from "./BoardPane";
import { GridPane } from "./GridPane";
import { RowPeekPanel } from "./RowPanel";
import { NewView, ViewOptions } from "./ViewMenus";

export interface TableViewProps {
  workspaceId: string;
  tableId: string;
  pages: readonly PageSummary[];
  pageActions: ReturnType<typeof usePageActions>;
  /** Open an address in the app without a reload. */
  open: (href: string) => void;
  /** The view an inline table shows first. */
  initialView?: string | undefined;
  /** In a page's body: its shortcuts wait until the table has the focus. */
  inline?: boolean | undefined;
}

/**
 * A table: its views as tabs, one toolbar row, and the grid or the board of
 * the current view with the row peek beside it. A table the viewer cannot
 * open is "Something you can't open", and nothing more.
 */
export function TableView(props: TableViewProps) {
  const { t } = useTranslation("pages");
  const table = useTable(props.workspaceId, props.tableId);
  if (table.isPending) {
    return <SkeletonList rows={4} />;
  }
  if (table.isError) {
    if (errorStatus(table.error) === 404) {
      return <DocCard icon="table" title={t("mention.closed")} state="closed" />;
    }
    return (
      <EmptyState
        title={t("tables.error.title")}
        visor="thinking"
        action={
          <Button
            onClick={() => {
              void table.refetch();
            }}
          >
            {t("error.retry")}
          </Button>
        }
      >
        {t("error.body")}
      </EmptyState>
    );
  }
  return <LoadedTable {...props} detail={table.data} />;
}

const DEFAULT_STATUS = [
  { key: "todo", color: "gray", group: "todo" },
  { key: "doing", color: "blue", group: "doing" },
  { key: "done", color: "green", group: "done" },
] as const;

function LoadedTable({
  workspaceId,
  tableId,
  pages,
  pageActions,
  open,
  initialView,
  inline = false,
  detail,
}: TableViewProps & { detail: TableDetail }) {
  const { t, i18n } = useTranslation("pages");
  const members = useMembers(workspaceId).data ?? [];
  const actions = useTableActions(workspaceId, tableId);
  const canWrite = detail.page.role !== "reader";

  const [chosen, setChosen] = useState(initialView);
  const view = detail.views.find((candidate) => candidate.id === chosen) ?? detail.views[0];
  const [editing, setEditing] = useState<{ position: CellPos; seed?: string | undefined } | null>(
    null,
  );
  const [reveal, setReveal] = useState<CellPos | undefined>();
  const [activeRow, setActiveRow] = useState<string | undefined>();
  const [peek, setPeek] = useState<string | undefined>();
  const [filterOpen, setFilterOpen] = useState(false);
  const [filterFocus, setFilterFocus] = useState<string | undefined>();
  const [focused, setFocused] = useState(false);

  const query = useTableRows(workspaceId, tableId, view?.id);
  const rows = useMemo<RowView[]>(
    () => query.data?.pages.flatMap((page) => page.rows) ?? [],
    [query.data],
  );
  const firstPage = query.data?.pages[0];

  const ctx: CellContext = { workspaceId, locale: i18n.language, members, open };
  const here = { id: tableId, title: detail.page.title };
  const tables = pages
    .filter((page) => page.kind === "table" && page.trashed_at === null)
    .map((page) => ({ id: page.id, title: page.title }));
  const titleColumn = detail.properties.find((property) => property.type === "title");

  // An inline table's keys wait until the table has the focus (a page may hold several).
  const box = useRef<HTMLElement>(null);
  useEffect(() => {
    const element = box.current;
    if (element === null) {
      return;
    }
    const into = () => {
      setFocused(true);
    };
    const out = (event: FocusEvent) => {
      if (!(event.relatedTarget instanceof Node && element.contains(event.relatedTarget))) {
        setFocused(false);
      }
    };
    element.addEventListener("focusin", into);
    element.addEventListener("focusout", out);
    return () => {
      element.removeEventListener("focusin", into);
      element.removeEventListener("focusout", out);
    };
  }, []);
  const shortcutsOn = !inline || focused;
  const addRow = async (cells?: { property: string; value: unknown }[]) => {
    const made = await actions.addRow(cells === undefined ? {} : { cells });
    if (made === undefined) {
      return;
    }
    if (view?.type === "board") {
      setPeek(made.id);
    } else if (titleColumn !== undefined) {
      const position = { rowId: made.id, columnId: titleColumn.id };
      setReveal(position);
      setEditing({ position });
    }
  };
  useShortcut(
    "pages.table.new_row",
    () => {
      void addRow();
    },
    { enabled: canWrite && shortcutsOn },
  );
  useShortcut(
    "pages.table.open_row",
    () => {
      if (activeRow !== undefined) {
        setPeek(activeRow);
      }
    },
    { enabled: shortcutsOn && view?.type !== "board" },
  );

  if (view === undefined) {
    return null;
  }
  const config = viewConfig(view);
  const board = view.type === "board";
  const columns = columnsOf(detail.properties, config);
  const peeked = peek === undefined ? undefined : rows.find((row) => row.id === peek);

  const save = (change: Partial<typeof config>, label?: string) => {
    void actions.updateView(view, { config: configJson({ ...config, ...change }, board) }, label);
  };

  const newProperty = (
    name: string,
    type: PropertyType,
    extra: { table?: string; pairedName?: string },
  ) => {
    const body: NewProperty = { name, type };
    if (type === "status") {
      body.config = {
        options: DEFAULT_STATUS.map((option) => ({
          name: t(`tables.status.default.${option.key}`),
          color: option.color,
          group: option.group,
        })),
      };
    } else if (type === "relation" && extra.table !== undefined) {
      body.config = { table: extra.table };
      if (extra.pairedName !== undefined) {
        body.paired_name = extra.pairedName;
      }
    }
    void actions.addProperty(body);
  };

  const conditions = flatFilter(config.filter);
  const valueLabel = (property: PropertyView, condition: Condition): string => {
    const value = condition.value;
    if (typeof value === "string") {
      if (
        property.type === "select" ||
        property.type === "status" ||
        property.type === "multi_select"
      ) {
        return optionById(property, value)?.name ?? "";
      }
      if (property.type === "person" || property.type === "created" || property.type === "edited") {
        return memberName(members, value) ?? value;
      }
      return property.type === "relation" ? "" : value;
    }
    if (Array.isArray(value)) {
      return idsOf(value)
        .map((id) => optionById(property, id)?.name ?? "")
        .filter((name) => name !== "")
        .join(", ");
    }
    if (typeof value === "boolean") {
      return t(value ? "tables.filter.checked" : "tables.filter.unchecked");
    }
    return "";
  };
  const chips: ReactNode[] = conditions.nested
    ? [
        <FilterChip
          key="nested"
          removeLabel={t("tables.filter.clear")}
          onRemove={() => {
            save({ filter: undefined });
          }}
          onOpen={() => {
            setFilterOpen(true);
          }}
        >
          {t("tables.filter.nestedChip")}
        </FilterChip>,
      ]
    : conditions.conditions.flatMap((condition, index) => {
        const property = detail.properties.find((candidate) => candidate.id === condition.property);
        if (property === undefined) {
          return [];
        }
        return (
          <FilterChip
            key={`${condition.property}-${String(index)}`}
            removeLabel={t("tables.filter.removeChip", { property: property.name })}
            onRemove={() => {
              save({
                filter: filterOf({
                  mode: conditions.mode,
                  conditions: conditions.conditions.filter((_, at) => at !== index),
                }),
              });
            }}
            onOpen={() => {
              setFilterFocus(undefined);
              setFilterOpen(true);
            }}
          >
            {`${property.name} ${t(`tables.operator.${condition.operator}`)} ${valueLabel(property, condition)}`.trim()}
          </FilterChip>
        );
      });

  const applyFilter = (filter: FilterNode | undefined) => {
    save({ filter });
  };

  const panelId = `table-${tableId}-panel`;
  const toolbar = (
    <TableToolbar
      tabs={
        <TableTabs
          label={t("tables.view.label")}
          panelId={panelId}
          tabs={detail.views.map((candidate) => ({
            id: candidate.id,
            label: candidate.name,
            icon: candidate.type === "board" ? "board" : "table",
            current: candidate.id === view.id,
            onSelect: () => {
              setChosen(candidate.id);
              setEditing(null);
            },
            after: (
              <ViewOptions
                view={candidate}
                columns={columns}
                properties={detail.properties}
                groupBy={config.groupBy}
                canWrite={canWrite}
                canRemove={detail.views.length > 1}
                onRename={(name) => {
                  void actions.updateView(candidate, { name });
                }}
                onToggle={(property, visible) => {
                  save({
                    properties: specsOf(
                      columns.map((column) =>
                        column.property.id === property ? { ...column, visible } : column,
                      ),
                    ),
                  });
                }}
                onGroupBy={(property) => {
                  save({ groupBy: property });
                }}
                onRemove={() => {
                  void actions.removeView(candidate).then((removed) => {
                    if (removed) {
                      setChosen(undefined);
                    }
                  });
                }}
              />
            ),
          }))}
          trailing={
            canWrite ? (
              <NewView
                properties={detail.properties}
                onCreate={(name, type: TableViewType, groupBy) => {
                  void actions
                    .createView({
                      name,
                      type,
                      config:
                        type === "board" && groupBy !== undefined ? { group_by: groupBy } : {},
                    })
                    .then((made) => {
                      if (made !== undefined) {
                        setChosen(made.id);
                      }
                    });
                }}
              />
            ) : undefined
          }
        />
      }
      chips={chips.length === 0 ? undefined : chips}
      actions={
        <>
          <Popover
            label={t("tables.filter.label")}
            placement="bottom-end"
            layer="top"
            open={filterOpen}
            onOpenChange={(next) => {
              setFilterOpen(next);
              if (!next) {
                setFilterFocus(undefined);
              }
            }}
            renderTrigger={(trigger) => (
              <ToolbarButton
                {...trigger}
                icon="filter"
                label={t("tables.filter.label")}
                count={conditions.conditions.length}
              />
            )}
          >
            {(close) => (
              <FilterBuilder
                ctx={ctx}
                properties={detail.properties}
                filter={config.filter}
                pages={pages}
                focus={filterFocus}
                onApply={applyFilter}
                onClose={close}
              />
            )}
          </Popover>
          <Popover
            label={t("tables.sort.label")}
            placement="bottom-end"
            layer="top"
            renderTrigger={(trigger) => (
              <ToolbarButton
                {...trigger}
                icon="sort"
                label={t("tables.sort.label")}
                count={config.sorts.length}
              />
            )}
          >
            {(close) => (
              <SortBuilder
                properties={detail.properties}
                sorts={config.sorts}
                onApply={(sorts) => {
                  save({ sorts });
                }}
                onClose={close}
              />
            )}
          </Popover>
          {canWrite ? (
            <ToolbarButton
              primary
              icon="plus"
              label={t("tables.rows.new")}
              keys="N R"
              onClick={() => {
                void addRow();
              }}
            />
          ) : null}
        </>
      }
    />
  );

  return (
    <section
      ref={box}
      aria-label={detail.page.title === "" ? t("untitled") : detail.page.title}
      className="flex min-w-0 flex-col gap-3"
    >
      {inline ? (
        <MentionChip
          kind="page"
          label={detail.page.title === "" ? t("untitled") : detail.page.title}
          href={`/w/${workspaceId}/pages/${tableId}`}
          onOpen={(event) => {
            if (!event.metaKey && !event.ctrlKey && event.button === 0) {
              event.preventDefault();
              open(`/w/${workspaceId}/pages/${tableId}`);
            }
          }}
        />
      ) : null}
      {toolbar}
      <div id={panelId} role="tabpanel" className="min-w-0">
        {query.isPending ? <SkeletonList rows={4} /> : null}
        {query.isError ? (
          <EmptyState
            title={t("tables.error.rows")}
            visor="thinking"
            action={
              <Button
                onClick={() => {
                  void query.refetch();
                }}
              >
                {t("error.retry")}
              </Button>
            }
          >
            {t("error.body")}
          </EmptyState>
        ) : null}
        {query.isSuccess && board ? (
          <BoardPane
            ctx={ctx}
            view={view}
            properties={detail.properties}
            rows={rows}
            groups={firstPage?.groups ?? []}
            total={firstPage?.matching ?? rows.length}
            actions={actions}
            canWrite={canWrite}
            complete={!query.hasNextPage}
            onOpenRow={setPeek}
            onGroupBy={(property) => {
              save({ groupBy: property });
            }}
            onNewRow={(group) => {
              const groupBy = config.groupBy;
              void addRow(
                group === undefined || groupBy === undefined
                  ? undefined
                  : [{ property: groupBy, value: group }],
              );
            }}
          />
        ) : null}
        {query.isSuccess && !board ? (
          <GridPane
            ctx={ctx}
            view={view}
            properties={detail.properties}
            rows={rows}
            total={firstPage?.matching ?? rows.length}
            summaries={firstPage?.summaries ?? []}
            pages={pages}
            tables={tables}
            here={here}
            actions={actions}
            canWrite={canWrite}
            editing={editing}
            onEditing={setEditing}
            reveal={reveal}
            onActiveRow={setActiveRow}
            onOpenRow={setPeek}
            onFilter={(property) => {
              setFilterFocus(property);
              setFilterOpen(true);
            }}
            hasMore={query.hasNextPage}
            loadingMore={query.isFetchingNextPage}
            onLoadMore={() => {
              void query.fetchNextPage();
            }}
            upload={(row, file) => pageActions.calls.upload(row, file)}
            onNewColumn={newProperty}
            onNewRow={() => {
              void addRow();
            }}
          />
        ) : null}
      </div>
      {peeked === undefined ? null : (
        <RowPeekPanel
          key={peeked.id}
          ctx={ctx}
          tableTitle={detail.page.title}
          properties={detail.properties}
          row={peeked}
          actions={actions}
          pageActions={pageActions}
          pages={pages}
          readOnly={!canWrite}
          canEditOptions={canWrite}
          onClose={() => {
            setPeek(undefined);
          }}
          onOpenAsPage={() => {
            setPeek(undefined);
            open(`/w/${workspaceId}/pages/${peeked.id}`);
          }}
        />
      )}
    </section>
  );
}
