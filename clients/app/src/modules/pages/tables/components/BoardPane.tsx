import { useTranslation } from "react-i18next";

import { Board, CellRow, EmptyState, Select, type BoardGroup } from "~/lib/ui";

import { CellDisplay } from "../cells/CellDisplay";
import type { CellContext } from "../cells/context";
import {
  colorOf,
  columnsOf,
  formatNumber,
  groupable,
  idsOf,
  isEmptyValue,
  optionsOf,
  viewConfig,
} from "../model";
import type { TableActions } from "../use-table-actions";
import type { GroupSize, PropertyView, RowView, TableViewView } from "../types";

export interface BoardPaneProps {
  ctx: CellContext;
  view: TableViewView;
  properties: readonly PropertyView[];
  rows: readonly RowView[];
  groups: readonly GroupSize[];
  total: number;
  actions: TableActions;
  canWrite: boolean;
  /** Every row is loaded: column sums are exact. */
  complete: boolean;
  onOpenRow: (row: string) => void;
  /** Choose the property a board without one groups by. */
  onGroupBy: (property: string) => void;
  onNewRow: (group: string | undefined) => void;
}

/** The cards of a board view in a column per option, with the count and the sum of each. */
export function BoardPane({
  ctx,
  view,
  properties,
  rows,
  groups,
  total,
  actions,
  canWrite,
  complete,
  onOpenRow,
  onGroupBy,
  onNewRow,
}: BoardPaneProps) {
  const { t } = useTranslation("pages");
  const config = viewConfig(view);
  const groupProperty = properties.find((property) => property.id === config.groupBy);
  const choices = properties.filter((property) => groupable(property.type));

  if (groupProperty === undefined || !groupable(groupProperty.type)) {
    return (
      <div className="flex flex-col gap-3 p-4">
        <EmptyState title={t("tables.board.noGroupTitle")} visor="unsure">
          {choices.length === 0 ? t("tables.board.noGroupBody") : t("tables.board.pickGroupBody")}
        </EmptyState>
        {choices.length === 0 || !canWrite ? null : (
          <Select
            label={t("tables.view.groupBy")}
            value=""
            options={[
              { value: "", label: t("tables.board.pickGroup") },
              ...choices.map((property) => ({ value: property.id, label: property.name })),
            ]}
            onChange={(value) => {
              if (value !== "") {
                onGroupBy(value);
              }
            }}
          />
        )}
      </div>
    );
  }

  const options = optionsOf(groupProperty);
  const sizes = new Map<string | null, number>(
    groups.map((group) => [group.option ?? null, group.rows]),
  );
  const hidden = new Set(config.hiddenGroups);
  const columns = columnsOf(properties, config);
  // The cards show the view's chosen properties, else the first few that have something.
  const cardProperties =
    config.cardProperties.length > 0
      ? config.cardProperties
          .map((id) => properties.find((property) => property.id === id))
          .filter((property): property is PropertyView => property !== undefined)
      : columns
          .filter((column) => column.visible)
          .map((column) => column.property)
          .filter((property) => property.type !== "title" && property.id !== groupProperty.id)
          .slice(0, 3);
  const sumProperty = config.summaries.find((summary) => summary.kind === "sum")?.property;
  const summed = properties.find((property) => property.id === sumProperty);

  const groupOf = (row: RowView): string | undefined =>
    idsOf(row.cells[groupProperty.id]?.value)[0];

  const boardGroups: BoardGroup[] = [
    ...options.map((option) => ({
      id: option.id,
      name: option.name,
      color: colorOf(option.color),
    })),
    {
      id: "",
      name: t("tables.board.none", { property: groupProperty.name }),
      color: "default" as const,
    },
  ]
    .filter((group) => !hidden.has(group.id === "" ? "none" : group.id))
    .map((group) => {
      const cards = rows.filter((row) => {
        const id = groupOf(row);
        return group.id === ""
          ? id === undefined || !options.some((o) => o.id === id)
          : id === group.id;
      });
      const count = sizes.get(group.id === "" ? null : group.id) ?? cards.length;
      const sum =
        summed === undefined || !complete
          ? undefined
          : cards.reduce((acc, row) => {
              const value = Number(row.cells[summed.id]?.value);
              return Number.isFinite(value) ? acc + value : acc;
            }, 0);
      return {
        ...group,
        count,
        ...(sum === undefined || summed === undefined
          ? {}
          : { summary: formatNumber(String(sum), summed, ctx.locale) }),
        cards: cards.map((row) => ({
          id: row.id,
          title: row.title === "" ? t("untitled") : row.title,
          onOpen: () => {
            onOpenRow(row.id);
          },
          body: (
            <CellRow>
              {cardProperties
                .filter((property) => !isEmptyValue(row.cells[property.id]?.value))
                .map((property) => (
                  <CellDisplay
                    key={property.id}
                    ctx={ctx}
                    property={property}
                    row={row}
                    value={row.cells[property.id]?.value}
                  />
                ))}
            </CellRow>
          ),
        })),
      };
    })
    // The empty group only shows when something is in it.
    .filter((group) => group.id !== "" || group.count > 0);

  return (
    <Board
      label={view.name}
      groups={boardGroups}
      readOnly={!canWrite}
      moveText={t("tables.board.moveTo")}
      moveLabel={(title) => t("tables.board.moveCard", { title })}
      onMove={(cardId, groupId) => {
        const row = rows.find((candidate) => candidate.id === cardId);
        if (row !== undefined) {
          void actions.setCell(row, groupProperty, groupId === "" ? null : groupId);
        }
      }}
      onAdd={(groupId) => {
        onNewRow(groupId === "" ? undefined : groupId);
      }}
      addLabel={(name) => t("tables.board.addCard", { group: name })}
      more={
        complete ? undefined : (
          <span>{t("tables.rows.loadedOf", { shown: rows.length, total })}</span>
        )
      }
    />
  );
}
