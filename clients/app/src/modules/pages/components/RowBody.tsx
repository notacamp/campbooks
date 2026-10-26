import { PageEditor } from "../editor/PageEditor";
import { usePageDocument } from "../editor/use-page-document";
import { usePage } from "../api/queries";
import type { RowBodyProps } from "../tables/components/row-body";

/** A row's page, made for the row peek (see `RowBodyContext`). */
export function renderRowBody(props: RowBodyProps) {
  return <RowBody {...props} />;
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
