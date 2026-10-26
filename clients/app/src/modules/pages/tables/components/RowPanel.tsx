import { use, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  CellButton,
  CellEmpty,
  IconButton,
  PropertyLine,
  PropertyList,
  RowPeek,
  TitleField,
  TypeGlyph,
} from "~/lib/ui";

import type { usePageActions } from "../../components/use-page-actions";
import type { PageSummary, UploadedFile } from "../../types";
import { CellDisplay } from "../cells/CellDisplay";
import { CellEditor } from "../cells/CellEditor";
import type { CellContext } from "../cells/context";
import { editable as typeEditable, isEmptyValue } from "../model";
import type { TableActions } from "../use-table-actions";
import { glyphOf } from "../type-info";
import type { PropertyType, PropertyView, RowView } from "../types";

import { RowBodyContext } from "./row-body";

/** The types whose values can hold links (or chips that open): they are shown beside their Edit button. */
const HOLDS_LINKS = new Set<PropertyType>([
  "text",
  "url",
  "email",
  "files",
  "relation",
  "campbooks_link",
]);

export interface RowPropertiesProps {
  ctx: CellContext;
  properties: readonly PropertyView[];
  row: RowView;
  actions: TableActions;
  pages: readonly PageSummary[];
  /** The member may not change the row. */
  readOnly: boolean;
  canEditOptions: boolean;
  upload: (row: string, file: File) => Promise<UploadedFile>;
}

/** The properties of one row, one under another; each opens its own editor in place. */
export function RowProperties({
  ctx,
  properties,
  row,
  actions,
  pages,
  readOnly,
  canEditOptions,
  upload,
}: RowPropertiesProps) {
  const { t } = useTranslation("pages");
  const [editing, setEditing] = useState<string | undefined>();
  const shown = properties.filter((property) => property.type !== "title");

  return (
    <PropertyList>
      {shown.map((property) => {
        const cell = row.cells[property.id];
        const value = cell?.value;
        const writable = !readOnly && typeEditable(property.type);
        const done = () => {
          setEditing(undefined);
        };
        let content;
        if (editing === property.id && writable) {
          content = (
            <CellEditor
              ctx={ctx}
              property={property}
              row={row}
              value={value}
              pages={pages}
              canEditOptions={canEditOptions}
              onCancel={done}
              onCommit={(next) => {
                done();
                void actions.setCell(row, property, next);
              }}
              onOptions={(next, previous) => actions.setOptions(property, next, previous)}
              upload={upload}
            />
          );
        } else if (property.type === "checkbox") {
          content = (
            <input
              type="checkbox"
              aria-label={property.name}
              checked={value === true}
              disabled={!writable}
              onChange={(event) => {
                void actions.setCell(row, property, event.target.checked);
              }}
            />
          );
        } else if (writable) {
          const edit = () => {
            setEditing(property.id);
          };
          const label = t("tables.cell.edit", { property: property.name });
          const shown = isEmptyValue(value) ? (
            <CellEmpty>{t("tables.cell.empty")}</CellEmpty>
          ) : (
            <CellDisplay ctx={ctx} property={property} row={row} value={value} />
          );
          // A value with links in it cannot sit inside a button: its Edit button stands beside it.
          content = HOLDS_LINKS.has(property.type) ? (
            <>
              {shown}
              <IconButton icon="edit" label={label} onClick={edit} />
            </>
          ) : (
            <CellButton aria-label={label} onClick={edit}>
              {shown}
            </CellButton>
          );
        } else {
          content = <CellDisplay ctx={ctx} property={property} row={row} value={value} />;
        }
        return (
          <PropertyLine
            key={property.id}
            name={property.name}
            glyph={<TypeGlyph glyph={glyphOf(property.type)} />}
          >
            {content}
          </PropertyLine>
        );
      })}
    </PropertyList>
  );
}

export interface RowPeekPanelProps {
  ctx: CellContext;
  tableTitle: string;
  properties: readonly PropertyView[];
  row: RowView;
  actions: TableActions;
  pageActions: ReturnType<typeof usePageActions>;
  pages: readonly PageSummary[];
  readOnly: boolean;
  canEditOptions: boolean;
  onClose: () => void;
  onOpenAsPage: () => void;
}

/** A row opened beside its table: title, properties, and its page. */
export function RowPeekPanel({
  ctx,
  tableTitle,
  properties,
  row,
  actions,
  pageActions,
  pages,
  readOnly,
  canEditOptions,
  onClose,
  onOpenAsPage,
}: RowPeekPanelProps) {
  const { t } = useTranslation("pages");
  const renderBody = use(RowBodyContext);
  const [title, setTitle] = useState(row.title);
  return (
    <RowPeek
      label={t("tables.peek.label")}
      crumb={t("tables.peek.crumb", { table: tableTitle === "" ? t("untitled") : tableTitle })}
      openAsPage={t("tables.peek.openAsPage")}
      onOpenAsPage={onOpenAsPage}
      closeLabel={t("tables.peek.close")}
      onClose={onClose}
      {...(readOnly
        ? {}
        : {
            trashLabel: t("tables.peek.trash"),
            onTrash: () => {
              void actions.trashRow(row).then((done) => {
                if (done) {
                  onClose();
                }
              });
            },
          })}
    >
      <TitleField
        label={t("tables.peek.title")}
        placeholder={t("untitled")}
        value={title}
        readOnly={readOnly}
        onChange={(event) => {
          setTitle(event.target.value);
        }}
        onBlur={() => {
          if (title !== row.title && !readOnly) {
            void actions.setTitle(row, title);
          }
        }}
      />
      <RowProperties
        ctx={ctx}
        properties={properties}
        row={row}
        actions={actions}
        pages={pages}
        readOnly={readOnly}
        canEditOptions={canEditOptions}
        upload={(rowId, file) => pageActions.calls.upload(rowId, file)}
      />
      {renderBody?.({
        workspaceId: ctx.workspaceId,
        rowId: row.id,
        pages,
        actions: pageActions,
        editable: !readOnly,
        open: ctx.open,
      })}
    </RowPeek>
  );
}
