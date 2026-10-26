import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  Checkbox,
  IconButton,
  MenuHeading,
  MenuItem,
  MenuList,
  MenuSeparator,
  Picker,
  Popover,
  TextField,
  HeaderButton,
  type PickerItem,
} from "~/lib/ui";

import { focusOnMount } from "../focus";
import { isRecord, showsWho, sortable } from "../model";
import { glyphOf } from "../type-info";
import {
  TYPE_GROUPS,
  type PropertyType,
  type PropertyView,
  type SortDirection,
  type TypeGroup,
} from "../types";

/** The types a member can make, in three groups: the menu of a new column and of a type change. */
export function TypeMenu({
  label,
  current,
  onPick,
}: {
  label: string;
  current?: PropertyType | undefined;
  onPick: (type: PropertyType) => void;
}) {
  const { t } = useTranslation("pages");
  const groups = Object.keys(TYPE_GROUPS) as TypeGroup[];
  return (
    <MenuList label={label}>
      {groups.map((group) => (
        <div key={group} role="group" aria-label={t(`tables.typeGroup.${group}`)}>
          <MenuHeading>{t(`tables.typeGroup.${group}`)}</MenuHeading>
          {TYPE_GROUPS[group].map((type) => (
            <MenuItem
              key={type}
              glyph={glyphOf(type)}
              hint={t(`tables.typeHint.${type}`)}
              checked={current === undefined ? undefined : current === type}
              onSelect={() => {
                onPick(type);
              }}
            >
              {t(`tables.type.${type}`)}
            </MenuItem>
          ))}
        </div>
      ))}
    </MenuList>
  );
}

export interface TableChoice {
  id: string;
  title: string;
}

export interface NewColumnProps {
  /** The tables a relation can point at (the ones the member may open). */
  tables: readonly TableChoice[];
  /** This table, for the "show it on the other table" option. */
  here: TableChoice;
  onCreate: (
    name: string,
    type: PropertyType,
    extra: { table?: string; pairedName?: string },
  ) => void;
}

/** The "+" at the end of the header: name the column, then pick what it holds. */
export function NewColumn({ tables, here, onCreate }: NewColumnProps) {
  const { t } = useTranslation("pages");
  const [name, setName] = useState("");
  const [step, setStep] = useState<"type" | "relation">("type");
  const [query, setQuery] = useState("");
  const [paired, setPaired] = useState(false);

  const reset = () => {
    setName("");
    setStep("type");
    setQuery("");
    setPaired(false);
  };

  return (
    <Popover
      label={t("tables.column.add")}
      placement="bottom-end"
      layer="top"
      onOpenChange={(open) => {
        if (!open) {
          reset();
        }
      }}
      renderTrigger={(trigger) => (
        <IconButton {...trigger} icon="plus" label={t("tables.column.add")} />
      )}
    >
      {(close) =>
        step === "type" ? (
          <div className="flex flex-col gap-1 p-2">
            <TextField
              label={t("tables.column.name")}
              placeholder={t("tables.column.namePlaceholder")}
              value={name}
              maxLength={100}
              onChange={(event) => {
                setName(event.target.value);
              }}
            />
            <TypeMenu
              label={t("tables.column.type")}
              onPick={(type) => {
                if (type === "relation") {
                  setStep("relation");
                  return;
                }
                close();
                onCreate(name.trim() === "" ? t(`tables.type.${type}`) : name.trim(), type, {});
              }}
            />
          </div>
        ) : (
          <div className="flex flex-col gap-2 p-2">
            <Picker
              label={t("tables.column.relatedTable")}
              searchLabel={t("tables.column.searchTables")}
              placeholder={t("tables.column.searchTables")}
              query={query}
              onQuery={setQuery}
              empty={t("tables.column.noTables")}
              items={tables
                .filter((table) => table.title.toLowerCase().includes(query.trim().toLowerCase()))
                .map((table): PickerItem => ({
                  id: table.id,
                  text: table.title === "" ? t("untitled") : table.title,
                  label: <span>{table.title === "" ? t("untitled") : table.title}</span>,
                }))}
              onPick={(table) => {
                close();
                onCreate(name.trim() === "" ? t("tables.type.relation") : name.trim(), "relation", {
                  table,
                  ...(paired ? { pairedName: here.title === "" ? t("untitled") : here.title } : {}),
                });
              }}
              footer={
                <Checkbox
                  label={t("tables.column.paired")}
                  checked={paired}
                  onChange={(event) => {
                    setPaired(event.target.checked);
                  }}
                />
              }
            />
            <Button
              variant="ghost"
              onClick={() => {
                setStep("type");
              }}
            >
              {t("tables.picker.back")}
            </Button>
          </div>
        )
      }
    </Popover>
  );
}

const NUMBER_FORMATS = [
  ["number", "number", undefined],
  ["percent", "percent", undefined],
  ["EUR", "currency", "EUR"],
  ["USD", "currency", "USD"],
  ["GBP", "currency", "GBP"],
] as const;

export interface ColumnMenuProps {
  property: PropertyView;
  /** The view's sort on this column. */
  sort: SortDirection | undefined;
  canWrite: boolean;
  /** The title stays: it cannot be hidden, retyped or deleted. */
  onRename: (name: string) => void;
  onType: (type: PropertyType) => void;
  onConfig: (config: Record<string, unknown>) => void;
  onSort: (direction: SortDirection | undefined) => void;
  onFilter: () => void;
  onHide: () => void;
  onDelete: () => void;
}

/** A column header: its name opens the column's menu. */
export function ColumnMenu({
  property,
  sort,
  canWrite,
  onRename,
  onType,
  onConfig,
  onSort,
  onFilter,
  onHide,
  onDelete,
}: ColumnMenuProps) {
  const { t } = useTranslation("pages");
  const [step, setStep] = useState<"menu" | "rename" | "type">("menu");
  const [name, setName] = useState(property.name);
  const isTitle = property.type === "title";
  const config = isRecord(property.config) ? property.config : {};
  const label = t("tables.column.menu", { property: property.name });

  return (
    <Popover
      label={label}
      placement="bottom-start"
      layer="top"
      onOpenChange={() => {
        setStep("menu");
        setName(property.name);
      }}
      renderTrigger={(trigger) => (
        <HeaderButton
          {...trigger}
          glyph={glyphOf(property.type)}
          trailing={
            sort === undefined ? null : (
              <span role="img" aria-label={t(`tables.sort.${sort}`)}>
                {sort === "asc" ? "↑" : "↓"}
              </span>
            )
          }
        >
          {property.name}
        </HeaderButton>
      )}
    >
      {(close) => {
        if (step === "rename") {
          return (
            <form
              className="flex flex-col gap-2 p-3"
              onSubmit={(event) => {
                event.preventDefault();
                if (name.trim() !== "" && name.trim() !== property.name) {
                  onRename(name.trim());
                }
                close();
              }}
            >
              <TextField
                label={t("tables.column.name")}
                value={name}
                maxLength={100}
                ref={focusOnMount}
                onChange={(event) => {
                  setName(event.target.value);
                }}
              />
              <Button type="submit" variant="primary" disabled={name.trim() === ""}>
                {t("tables.column.rename")}
              </Button>
            </form>
          );
        }
        if (step === "type") {
          return (
            <TypeMenu
              label={t("tables.column.type")}
              current={property.type}
              onPick={(type) => {
                close();
                if (type !== property.type) {
                  onType(type);
                }
              }}
            />
          );
        }
        return (
          <MenuList label={label}>
            {canWrite ? (
              <MenuItem
                icon="edit"
                onSelect={() => {
                  setStep("rename");
                }}
              >
                {t("tables.column.rename")}
              </MenuItem>
            ) : null}
            {canWrite && !isTitle ? (
              <MenuItem
                glyph={glyphOf(property.type)}
                hint={t(`tables.type.${property.type}`)}
                onSelect={() => {
                  setStep("type");
                }}
              >
                {t("tables.column.changeType")}
              </MenuItem>
            ) : null}
            {canWrite && property.type === "number" ? (
              <>
                <MenuHeading>{t("tables.column.format")}</MenuHeading>
                {NUMBER_FORMATS.map(([id, format, currency]) => (
                  <MenuItem
                    key={id}
                    checked={
                      config["format"] === format &&
                      (currency === undefined || config["currency"] === currency)
                    }
                    onSelect={() => {
                      close();
                      onConfig(currency === undefined ? { format } : { format, currency });
                    }}
                  >
                    {t(`tables.numberFormat.${id}`)}
                  </MenuItem>
                ))}
              </>
            ) : null}
            {canWrite && property.type === "date" ? (
              <MenuItem
                checked={config["time"] === true}
                onSelect={() => {
                  close();
                  onConfig({ time: config["time"] !== true });
                }}
              >
                {t("tables.column.includeTime")}
              </MenuItem>
            ) : null}
            {canWrite && (property.type === "created" || property.type === "edited") ? (
              <MenuItem
                checked={showsWho(property)}
                onSelect={() => {
                  close();
                  onConfig({ of: showsWho(property) ? "time" : "who" });
                }}
              >
                {t("tables.column.showsWho")}
              </MenuItem>
            ) : null}
            {sortable(property) ? (
              <>
                <MenuSeparator />
                <MenuItem
                  icon="sort"
                  checked={sort === "asc"}
                  onSelect={() => {
                    close();
                    onSort(sort === "asc" ? undefined : "asc");
                  }}
                >
                  {t("tables.sort.asc")}
                </MenuItem>
                <MenuItem
                  icon="sort"
                  checked={sort === "desc"}
                  onSelect={() => {
                    close();
                    onSort(sort === "desc" ? undefined : "desc");
                  }}
                >
                  {t("tables.sort.desc")}
                </MenuItem>
              </>
            ) : null}
            <MenuItem
              icon="filter"
              onSelect={() => {
                close();
                onFilter();
              }}
            >
              {t("tables.column.filter")}
            </MenuItem>
            {isTitle ? null : (
              <MenuItem
                icon="close"
                onSelect={() => {
                  close();
                  onHide();
                }}
              >
                {t("tables.column.hide")}
              </MenuItem>
            )}
            {canWrite && !isTitle ? (
              <>
                <MenuSeparator />
                <MenuItem
                  icon="trash"
                  danger
                  onSelect={() => {
                    close();
                    onDelete();
                  }}
                >
                  {t("tables.column.delete")}
                </MenuItem>
              </>
            ) : null}
          </MenuList>
        );
      }}
    </Popover>
  );
}
