import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, CellEmpty, Icon, IconButton, Panel, Popover, Text } from "~/lib/ui";

import { CellDisplay } from "../cells/CellDisplay";
import type { CellContext } from "../cells/context";
import { isEmptyValue } from "../model";
import type { PropertyView, RowView } from "../types";

import type { TableConflict, TableConflictKeep } from "./model";

export interface CellConflictProps {
  ctx: CellContext;
  conflict: TableConflict;
  property: PropertyView;
  row: RowView;
  /** Settle it; whether it was. Without it (a reader) the card only shows both versions. */
  onChoose?: ((keep: TableConflictKeep) => Promise<boolean>) | undefined;
}

function Version({
  label,
  ctx,
  property,
  row,
  value,
}: {
  label: string;
  ctx: CellContext;
  property: PropertyView;
  row: RowView;
  value: unknown;
}) {
  const { t } = useTranslation("pages");
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-2">
      <Text as="span" tone="secondary" size="sm">
        {label}
      </Text>
      <div className="min-w-0 flex-1 break-words">
        {isEmptyValue(value) ? (
          <CellEmpty>{t("tables.cell.empty")}</CellEmpty>
        ) : (
          <CellDisplay ctx={ctx} property={property} row={row} value={value} />
        )}
      </div>
    </div>
  );
}

/**
 * A cell changed here and in Notion: Notion's value under ours, with Keep
 * mine and Use Notion's. Neither goes to Notion until one is chosen; every
 * other cell of the row merged on its own.
 */
export function CellConflict({ ctx, conflict, property, row, onChoose }: CellConflictProps) {
  const { t, i18n } = useTranslation("pages");
  const [busy, setBusy] = useState<TableConflictKeep | undefined>();
  const when = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(conflict.theirs_at));
  const choose = (keep: TableConflictKeep) => {
    if (onChoose === undefined) {
      return;
    }
    setBusy(keep);
    void onChoose(keep).then(() => {
      setBusy(undefined);
    });
  };
  return (
    <Panel label={t("notion.table.cell.label", { property: property.name })}>
      <div className="flex flex-wrap items-center gap-2">
        <Icon name="warning" />
        <Text as="span" weight="semibold" className="min-w-0 flex-1">
          {t("notion.table.cell.changed", { property: property.name })}
        </Text>
      </div>
      <Text tone="secondary" size="sm">
        {t("notion.table.cell.when", { when })}
      </Text>
      <Version
        label={t("notion.table.cell.mine")}
        ctx={ctx}
        property={property}
        row={row}
        value={conflict.ours}
      />
      <Version
        label={t("notion.table.cell.theirs")}
        ctx={ctx}
        property={property}
        row={row}
        value={conflict.theirs}
      />
      {onChoose === undefined ? null : (
        <div
          className="flex flex-wrap gap-2"
          role="group"
          aria-label={t("notion.table.cell.choices", { property: property.name })}
        >
          <Button
            variant="primary"
            disabled={busy !== undefined}
            aria-label={t("notion.table.cell.keepMineFor", { property: property.name })}
            onClick={() => {
              choose("ours");
            }}
          >
            {t("notion.conflict.mine")}
          </Button>
          <Button
            variant="secondary"
            disabled={busy !== undefined}
            aria-label={t("notion.table.cell.useNotionFor", { property: property.name })}
            onClick={() => {
              choose("theirs");
            }}
          >
            {t("notion.conflict.notion")}
          </Button>
        </div>
      )}
    </Panel>
  );
}

/** In a grid cell: a button that opens the conflict, so the cell stays one line. */
export function CellConflictFlag(props: CellConflictProps) {
  const { t } = useTranslation("pages");
  const label = t("notion.table.cell.flag", { property: props.property.name });
  return (
    <Popover
      label={label}
      placement="bottom-start"
      layer="top"
      renderTrigger={(trigger) => <IconButton {...trigger} icon="warning" label={label} />}
    >
      {(close) => (
        <div className="w-72 flex flex-col p-1">
          <CellConflict
            {...props}
            onChoose={
              props.onChoose === undefined
                ? undefined
                : async (keep) => {
                    const done = await props.onChoose?.(keep);
                    if (done === true) {
                      close();
                    }
                    return done === true;
                  }
            }
          />
        </div>
      )}
    </Popover>
  );
}
