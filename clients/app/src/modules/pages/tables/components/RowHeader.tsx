import { useTranslation } from "react-i18next";

import { useMembers } from "~/modules/workspaces";

import type { usePageActions } from "../../components/use-page-actions";
import type { PageSummary } from "../../types";
import type { CellContext } from "../cells/context";
import { useRowsById, useTable } from "../api";
import { useTableActions } from "../use-table-actions";

import { RowProperties } from "./RowPanel";

export interface RowHeaderProps {
  workspaceId: string;
  /** The row, a page. */
  rowId: string;
  /** Its table: the row page's parent. */
  tableId: string;
  pages: readonly PageSummary[];
  pageActions: ReturnType<typeof usePageActions>;
  open: (href: string) => void;
}

/** A row opened as a page: its properties, under the title and over the body. */
export function RowHeader({
  workspaceId,
  rowId,
  tableId,
  pages,
  pageActions,
  open,
}: RowHeaderProps) {
  const { i18n } = useTranslation("pages");
  const table = useTable(workspaceId, tableId);
  const found = useRowsById(workspaceId, tableId, [rowId]);
  const members = useMembers(workspaceId).data ?? [];
  const actions = useTableActions(workspaceId, tableId);
  const row = found.data?.rows[0];
  if (table.data === undefined || row === undefined) {
    return null;
  }
  const ctx: CellContext = { workspaceId, locale: i18n.language, members, open };
  const canWrite = table.data.page.role !== "reader";
  return (
    <RowProperties
      ctx={ctx}
      properties={table.data.properties}
      row={row}
      actions={actions}
      pages={pages}
      readOnly={!canWrite}
      canEditOptions={canWrite}
      upload={(id, file) => pageActions.calls.upload(id, file)}
    />
  );
}
