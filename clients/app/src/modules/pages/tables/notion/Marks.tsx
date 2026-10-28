import { useTranslation } from "react-i18next";

import { ColumnMark, Text } from "~/lib/ui";

import type { PropertyView } from "../types";

import { useSyncedTable } from "./context";
import { markOf } from "./model";

/**
 * What a column of a table kept in step with Notion says about itself: a lock
 * with the reason for what only Notion changes (a formula, a rollup, people),
 * "Campbooks only" for what is never sent. Nothing for an ordinary column.
 */
export function NotionMark({ property }: { property: PropertyView }) {
  const { t } = useTranslation("pages");
  const synced = useSyncedTable();
  const mark = markOf(property);
  if (mark.kind === "locked") {
    return <ColumnMark icon="lock" label={t(`notion.table.lock.${mark.lock}`)} />;
  }
  if (mark.kind === "campbooks_only" && synced.syncing) {
    return <ColumnMark icon="pin" label={t("notion.table.campbooksOnly.reason")} />;
  }
  return null;
}

/** The sentence under a column's name in its menu: why it is Notion's, or stays here. */
export function NotionNote({ property }: { property: PropertyView }) {
  const { t } = useTranslation("pages");
  const synced = useSyncedTable();
  const mark = markOf(property);
  let text: string | undefined;
  if (mark.kind === "locked") {
    text = t(`notion.table.lockNote.${mark.lock}`, { name: property.name });
  } else if (mark.kind === "campbooks_only" && synced.syncing) {
    text = t("notion.table.campbooksOnly.note", { name: property.name });
  }
  return text === undefined ? null : (
    <div className="p-2">
      <Text tone="secondary" size="sm">
        {text}
      </Text>
    </div>
  );
}

/** What changing options of a synced table comes to: adding goes to Notion, the rest stays here. */
export function OptionsNote() {
  const { t } = useTranslation("pages");
  const synced = useSyncedTable();
  return synced.syncing ? (
    <Text tone="secondary" size="xs">
      {t("notion.table.stays.options")}
    </Text>
  ) : null;
}
