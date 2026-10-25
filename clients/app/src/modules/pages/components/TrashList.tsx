import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, EmptyState, InlineConfirm, List, ListRow, Text } from "~/lib/ui";

import { useTrash } from "../api/queries";
import type { PageSummary } from "../types";

import type { usePageActions } from "./use-page-actions";

export interface TrashListProps {
  workspaceId: string;
  actions: ReturnType<typeof usePageActions>;
}

/**
 * What was deleted in the last 30 days, each the root of what went with it.
 * Restore brings it back with its sub-pages; Delete for good asks first and
 * cannot be undone.
 */
export function TrashList({ workspaceId, actions }: TrashListProps) {
  const { t, i18n } = useTranslation("pages");
  const trash = useTrash(workspaceId, true);
  const [confirming, setConfirming] = useState<string | undefined>();
  const pages = trash.data ?? [];
  const when = (page: PageSummary) =>
    page.trashed_at === null || page.trashed_at === undefined
      ? ""
      : new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" }).format(
          new Date(page.trashed_at),
        );
  if (trash.isSuccess && pages.length === 0) {
    return (
      <EmptyState title={t("trash.emptyTitle")} visor="happy" visorSize="lg">
        {t("trash.emptyBody")}
      </EmptyState>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <Text tone="tertiary" size="sm">
        {t("trash.note")}
      </Text>
      <List label={t("trash.label")}>
        {pages.map((page) => (
          <ListRow
            key={page.id}
            title={page.title === "" ? t("untitled") : page.title}
            meta={t("trash.deletedOn", { date: when(page) })}
            trailing={
              <>
                <Button
                  variant="ghost"
                  onClick={() => {
                    void actions.restore(page);
                  }}
                >
                  {t("trash.restore")}
                </Button>
                {page.role === "manager" ? (
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setConfirming(page.id);
                    }}
                  >
                    {t("trash.purge")}
                  </Button>
                ) : null}
              </>
            }
          >
            {confirming === page.id ? (
              <InlineConfirm
                confirmLabel={t("trash.purgeConfirm")}
                cancelLabel={t("trash.cancel")}
                onCancel={() => {
                  setConfirming(undefined);
                }}
                onConfirm={() => {
                  setConfirming(undefined);
                  void actions.purge(page);
                }}
              >
                {t("trash.purgeWarning", {
                  title: page.title === "" ? t("untitled") : page.title,
                })}
              </InlineConfirm>
            ) : null}
          </ListRow>
        ))}
      </List>
    </div>
  );
}
