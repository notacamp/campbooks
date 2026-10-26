import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  CellButton,
  CellEmpty,
  PropertyLine,
  PropertyList,
  RowPeek,
  TitleField,
  TypeGlyph,
} from "~/lib/ui";

import { PageEditor } from "../../editor/PageEditor";
import { usePageDocument } from "../../editor/use-page-document";
import { usePage } from "../../api/queries";
import type { usePageActions } from "../../components/use-page-actions";
import type { PageSummary, UploadedFile } from "../../types";
import { CellDisplay } from "../cells/CellDisplay";
import { CellEditor } from "../cells/CellEditor";
import type { CellContext } from "../cells/context";
import { editable as typeEditable, isEmptyValue } from "../model";
import type { TableActions } from "../use-table-actions";
import { glyphOf } from "../type-info";
import type { PropertyView, RowView } from "../types";

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
          content = (
            <CellButton
              aria-label={t("tables.cell.edit", { property: property.name })}
              onClick={() => {
                setEditing(property.id);
              }}
            >
              {isEmptyValue(value) ? (
                <CellEmpty>{t("tables.cell.empty")}</CellEmpty>
              ) : (
                <CellDisplay ctx={ctx} property={property} row={row} value={value} />
              )}
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

interface RowBodyProps {
  workspaceId: string;
  rowId: string;
  pages: readonly PageSummary[];
  actions: ReturnType<typeof usePageActions>;
  /** The row can be written (not a reader's). */
  editable: boolean;
  open: (href: string) => void;
}

/** A row's page: the notes, to-dos and mentions under its properties. */
function RowBody({ workspaceId, rowId, pages, actions, editable, open }: RowBodyProps) {
  const page = usePage(workspaceId, rowId);
  if (!page.isSuccess) {
    return null;
  }
  return (
    <RowBodyEditor
      key={page.data.id}
      workspaceId={workspaceId}
      page={page.data}
      pages={pages}
      actions={actions}
      editable={editable && page.data.trashed_at === null}
      open={open}
    />
  );
}

function RowBodyEditor({
  workspaceId,
  page,
  pages,
  actions,
  editable,
  open,
}: Omit<RowBodyProps, "rowId"> & { page: NonNullable<ReturnType<typeof usePage>["data"]> }) {
  const { doc } = usePageDocument({
    pageId: page.id,
    update: page.document.update,
    stateVector: page.document.state_vector,
    revision: page.revision,
    calls: actions.calls,
  });
  return (
    <PageEditor
      environment={{ workspaceId, pageId: page.id, open }}
      doc={doc}
      editable={editable}
      pages={pages}
      subPages={false}
      upload={(file) => actions.calls.upload(page.id, file)}
      createSubPage={() => Promise.reject(new Error("rows have no sub-pages"))}
    />
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
  const [title, setTitle] = useState(row.title);
  return (
    <RowPeek
      label={t("tables.peek.label")}
      crumb={t("tables.peek.crumb", { table: tableTitle === "" ? t("untitled") : tableTitle })}
      openAsPage={t("tables.peek.openAsPage")}
      onOpenAsPage={onOpenAsPage}
      closeLabel={t("tables.peek.close")}
      onClose={onClose}
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
      <RowBody
        workspaceId={ctx.workspaceId}
        rowId={row.id}
        pages={pages}
        actions={pageActions}
        editable={!readOnly}
        open={ctx.open}
      />
    </RowPeek>
  );
}
