import { useTranslation } from "react-i18next";

import { usePages } from "../../api/queries";
import { usePageActions } from "../../components/use-page-actions";

import { TableView } from "./TableView";

export interface InlineTableProps {
  workspaceId: string;
  tableId: string;
  /** The view the page shows. */
  viewId: string;
  open: (href: string) => void;
}

/**
 * A table in the body of a page: the table's own view, with its own rows
 * read for the viewer. It is the same screen as the table page, in a box.
 */
export function InlineTable({ workspaceId, tableId, viewId, open }: InlineTableProps) {
  const { t } = useTranslation("pages");
  const pages = usePages(workspaceId).data ?? [];
  const pageActions = usePageActions(workspaceId);
  if (tableId === "") {
    return <p>{t("mention.closed")}</p>;
  }
  return (
    <TableView
      inline
      workspaceId={workspaceId}
      tableId={tableId}
      initialView={viewId === "" ? undefined : viewId}
      pages={pages}
      pageActions={pageActions}
      open={open}
    />
  );
}
