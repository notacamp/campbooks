import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  CheckGroup,
  Checkbox,
  IconButton,
  Popover,
  Segmented,
  Select,
  TextField,
} from "~/lib/ui";

import { groupable } from "../model";
import type { Column } from "../model";
import type { PropertyView, TableViewType, TableViewView } from "../types";

export interface NewViewProps {
  properties: readonly PropertyView[];
  /** Make the view; a board needs the property it groups by. */
  onCreate: (name: string, type: TableViewType, groupBy: string | undefined) => void;
}

/** The "+" after the tabs: name a view, pick table or board. */
export function NewView({ properties, onCreate }: NewViewProps) {
  const { t } = useTranslation("pages");
  const groups = properties.filter((property) => groupable(property.type));
  const [name, setName] = useState("");
  const [type, setType] = useState<"table" | "board">("table");
  const [groupBy, setGroupBy] = useState<string | undefined>();
  const chosen = groupBy ?? groups[0]?.id;
  const board = type === "board";
  return (
    <Popover
      label={t("tables.view.add")}
      placement="bottom-start"
      layer="top"
      onOpenChange={(open) => {
        if (!open) {
          setName("");
          setType("table");
          setGroupBy(undefined);
        }
      }}
      renderTrigger={(trigger) => (
        <IconButton {...trigger} icon="plus" label={t("tables.view.add")} />
      )}
    >
      {(close) => (
        <form
          className="flex flex-col gap-3 p-3"
          onSubmit={(event) => {
            event.preventDefault();
            if (board && chosen === undefined) {
              return;
            }
            close();
            onCreate(
              name.trim() === "" ? t(`tables.view.default.${type}`) : name.trim(),
              type,
              board ? chosen : undefined,
            );
          }}
        >
          <TextField
            label={t("tables.view.name")}
            value={name}
            maxLength={100}
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
          <Segmented<"table" | "board">
            label={t("tables.view.type")}
            value={type}
            options={[
              { value: "table", label: t("tables.view.table") },
              { value: "board", label: t("tables.view.board") },
            ]}
            onChange={setType}
          />
          {board ? (
            groups.length === 0 ? (
              <p>{t("tables.view.needGroup")}</p>
            ) : (
              <Select
                label={t("tables.view.groupBy")}
                value={chosen ?? ""}
                options={groups.map((property) => ({ value: property.id, label: property.name }))}
                onChange={setGroupBy}
              />
            )
          ) : null}
          <Button type="submit" variant="primary" disabled={board && chosen === undefined}>
            {t("tables.view.create")}
          </Button>
        </form>
      )}
    </Popover>
  );
}

export interface ViewOptionsProps {
  view: TableViewView;
  /** Every column, in the view's order, shown or not. */
  columns: readonly Column[];
  /** The properties a board can group by. */
  properties: readonly PropertyView[];
  groupBy: string | undefined;
  canWrite: boolean;
  canRemove: boolean;
  onRename: (name: string) => void;
  onToggle: (property: string, visible: boolean) => void;
  onGroupBy: (property: string) => void;
  onRemove: () => void;
}

/** The current tab's menu: rename the view, choose its columns (or its groups), delete it. */
export function ViewOptions({
  view,
  columns,
  properties,
  groupBy,
  canWrite,
  canRemove,
  onRename,
  onToggle,
  onGroupBy,
  onRemove,
}: ViewOptionsProps) {
  const { t } = useTranslation("pages");
  const [name, setName] = useState(view.name);
  const groups = properties.filter((property) => groupable(property.type));
  return (
    <Popover
      label={t("tables.view.options", { view: view.name })}
      placement="bottom-start"
      layer="top"
      onOpenChange={() => {
        setName(view.name);
      }}
      renderTrigger={(trigger) => (
        <IconButton
          {...trigger}
          icon="more"
          label={t("tables.view.options", { view: view.name })}
        />
      )}
    >
      {(close) => (
        <div className="flex flex-col gap-3 p-3">
          {canWrite ? (
            <form
              className="flex flex-col gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (name.trim() !== "" && name.trim() !== view.name) {
                  onRename(name.trim());
                }
                close();
              }}
            >
              <TextField
                label={t("tables.view.name")}
                value={name}
                maxLength={100}
                onChange={(event) => {
                  setName(event.target.value);
                }}
              />
              <Button type="submit">{t("tables.view.rename")}</Button>
            </form>
          ) : null}
          {view.type === "board" && groups.length > 0 ? (
            <Select
              label={t("tables.view.groupBy")}
              value={groupBy ?? ""}
              disabled={!canWrite}
              options={groups.map((property) => ({ value: property.id, label: property.name }))}
              onChange={onGroupBy}
            />
          ) : null}
          <CheckGroup legend={t("tables.view.columns")}>
            {columns.map((column) => (
              <Checkbox
                key={column.property.id}
                label={column.property.name}
                checked={column.visible}
                disabled={!canWrite || column.property.type === "title"}
                onChange={(event) => {
                  onToggle(column.property.id, event.target.checked);
                }}
              />
            ))}
          </CheckGroup>
          {canWrite && canRemove ? (
            <Button
              variant="ghost"
              icon="trash"
              onClick={() => {
                close();
                onRemove();
              }}
            >
              {t("tables.view.remove")}
            </Button>
          ) : null}
        </div>
      )}
    </Popover>
  );
}
