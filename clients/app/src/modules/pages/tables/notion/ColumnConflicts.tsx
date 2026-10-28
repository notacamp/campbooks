import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, Icon, Panel, Text } from "~/lib/ui";

import {
  columnDifferences,
  columnVersion,
  type ColumnDifference,
  type ColumnVersion,
  type TableConflict,
  type TableConflictKeep,
} from "./model";

export interface ColumnConflictsProps {
  conflicts: readonly TableConflict[];
  /** Settle one; whether it was. Without it (a reader) the banner only says what differs. */
  onChoose?: ((conflict: string, keep: TableConflictKeep) => Promise<boolean>) | undefined;
}

/** A name on a button: long ones are cut so the buttons stay on one line of their own. */
const onButton = (name: string) => (name.length > 32 ? `${name.slice(0, 31)}…` : name);

function Difference({
  kind,
  ours,
  theirs,
}: {
  kind: ColumnDifference;
  ours: ColumnVersion;
  theirs: ColumnVersion;
}) {
  const { t } = useTranslation("pages");
  const type = (version: ColumnVersion) =>
    version.type === undefined ? "" : t(`tables.type.${version.type}`);
  const values =
    kind === "name"
      ? { mine: ours.name, theirs: theirs.name }
      : kind === "type"
        ? { mine: type(ours), theirs: type(theirs) }
        : { mine: String(ours.options), theirs: String(theirs.options) };
  return (
    <li>
      <Text as="span" tone="secondary" size="sm">
        {t(`notion.table.column.differs.${kind}`, values)}
      </Text>
    </li>
  );
}

function ColumnConflict({
  conflict,
  onChoose,
}: {
  conflict: TableConflict;
  onChoose?: ColumnConflictsProps["onChoose"];
}) {
  const { t } = useTranslation("pages");
  const [busy, setBusy] = useState<TableConflictKeep | undefined>();
  const ours = columnVersion(conflict.ours);
  const theirs = columnVersion(conflict.theirs);
  const differences = columnDifferences(ours, theirs);
  const named = differences.includes("name");
  const choose = (keep: TableConflictKeep) => {
    if (onChoose === undefined) {
      return;
    }
    setBusy(keep);
    void onChoose(conflict.id, keep).then(() => {
      setBusy(undefined);
    });
  };
  return (
    <Panel label={t("notion.table.column.label", { name: ours.name })}>
      <div className="flex flex-wrap items-center gap-2">
        <Icon name="warning" />
        <Text as="span" weight="semibold" className="min-w-0 flex-1 break-words">
          {t("notion.table.column.changed", { name: ours.name })}
        </Text>
      </div>
      <ul className="m-0 flex list-none flex-col gap-1 p-0">
        {differences.map((kind) => (
          <Difference key={kind} kind={kind} ours={ours} theirs={theirs} />
        ))}
      </ul>
      {onChoose === undefined ? null : (
        <div
          className="flex flex-wrap gap-2"
          role="group"
          aria-label={t("notion.table.column.choices", { name: ours.name })}
        >
          <Button
            variant="primary"
            disabled={busy !== undefined}
            onClick={() => {
              choose("ours");
            }}
          >
            {named
              ? t("notion.table.column.keepName", { name: onButton(ours.name) })
              : t("notion.conflict.mine")}
          </Button>
          <Button
            variant="secondary"
            disabled={busy !== undefined}
            onClick={() => {
              choose("theirs");
            }}
          >
            {named
              ? t("notion.table.column.useName", { name: onButton(theirs.name) })
              : t("notion.conflict.notion")}
          </Button>
        </div>
      )}
      <Text tone="tertiary" size="xs">
        {t("notion.table.column.footer")}
      </Text>
    </Panel>
  );
}

/**
 * Columns changed here and in Notion, above the table: both versions of each
 * (name, type, options) and a choice. A column removed on one side and edited
 * on the other is kept and never raised here.
 */
export function ColumnConflicts({ conflicts, onChoose }: ColumnConflictsProps) {
  const { t } = useTranslation("pages");
  if (conflicts.length === 0) {
    return null;
  }
  return (
    <section className="flex flex-col gap-2" aria-label={t("notion.table.column.banner")}>
      {conflicts.map((conflict) => (
        <ColumnConflict key={conflict.id} conflict={conflict} onChoose={onChoose} />
      ))}
    </section>
  );
}
