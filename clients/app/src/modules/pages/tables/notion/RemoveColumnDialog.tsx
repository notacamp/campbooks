import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Banner, Button, Dialog, Text } from "~/lib/ui";

export interface RemoveColumnDialogProps {
  /** The column asked about; none keeps the dialog closed. */
  name: string | undefined;
  /** How many rows the table has: each holds a value of the column. */
  rows: number;
  /** Remove it in both places; whether it was. */
  onConfirm: () => Promise<boolean>;
  onClose: () => void;
}

/**
 * "Remove from both places?": removing a column of a table kept in step with
 * Notion removes it from the Notion database too. It names the values that go
 * and the 30 days Campbooks keeps them for, and nothing is sent until the
 * member confirms.
 */
export function RemoveColumnDialog({ name, rows, onConfirm, onClose }: RemoveColumnDialogProps) {
  const { t } = useTranslation("pages");
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const close = () => {
    setFailed(false);
    onClose();
  };
  return (
    <Dialog
      open={name !== undefined}
      onClose={close}
      title={t("notion.table.remove.title", { name })}
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t("notion.table.remove.cancel")}
          </Button>
          <Button
            variant="primary"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              setFailed(false);
              void onConfirm().then((done) => {
                setBusy(false);
                if (done) {
                  onClose();
                } else {
                  setFailed(true);
                }
              });
            }}
          >
            {t("notion.table.remove.confirm")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Text tone="secondary">{t("notion.table.remove.body", { count: rows, name })}</Text>
        {failed ? (
          <Banner tone="danger" role="alert" title={t("notion.table.remove.failed")} />
        ) : null}
      </div>
    </Dialog>
  );
}
