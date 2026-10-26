import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  CheckGroup,
  Checkbox,
  DatePicker,
  IconButton,
  Segmented,
  Select,
  TextField,
} from "~/lib/ui";
import type { Member } from "~/modules/workspaces";

import type { PageSummary } from "../../types";
import { LinkChip } from "../cells/CellDisplay";
import type { CellContext } from "../cells/context";
import { LinkPicker, RowPicker } from "../cells/Pickers";
import {
  conditionReady,
  filterOf,
  filterable,
  flatFilter,
  isRecord,
  linksOf,
  operatorsFor,
  optionsOf,
  showsWho,
  sortable,
  startingValue,
  takesValue,
  type FlatFilter,
} from "../model";
import type {
  Condition,
  FilterNode,
  Operator,
  PropertyView,
  SortDirection,
  SortSpec,
} from "../types";

// ------------------------------------------------------------------- filters

interface ValueInputProps {
  ctx: CellContext;
  property: PropertyView;
  condition: Condition;
  members: readonly Member[];
  pages: readonly PageSummary[];
  onChange: (value: unknown) => void;
}

/** The input for a condition's value, as its property's type needs. */
function ValueInput({ ctx, property, condition, members, pages, onChange }: ValueInputProps) {
  const { t } = useTranslation("pages");
  const label = t("tables.filter.value");
  const text = typeof condition.value === "string" ? condition.value : "";
  switch (property.type) {
    case "select":
    case "status":
    case "multi_select": {
      const options = optionsOf(property);
      if (condition.operator === "is_any_of") {
        const chosen = Array.isArray(condition.value) ? (condition.value as string[]) : [];
        return (
          <CheckGroup legend={label}>
            {options.map((option) => (
              <Checkbox
                key={option.id}
                label={option.name}
                checked={chosen.includes(option.id)}
                onChange={(event) => {
                  onChange(
                    event.target.checked
                      ? [...chosen, option.id]
                      : chosen.filter((id) => id !== option.id),
                  );
                }}
              />
            ))}
          </CheckGroup>
        );
      }
      return (
        <Select
          label={label}
          hideLabel
          value={text}
          options={options.map((option) => ({ value: option.id, label: option.name }))}
          onChange={onChange}
        />
      );
    }
    case "checkbox": {
      return (
        <Select
          label={label}
          hideLabel
          value={condition.value === false ? "no" : "yes"}
          options={[
            { value: "yes", label: t("tables.filter.checked") },
            { value: "no", label: t("tables.filter.unchecked") },
          ]}
          onChange={(value) => {
            onChange(value === "yes");
          }}
        />
      );
    }
    case "date":
    case "created":
    case "edited": {
      if ((property.type === "created" || property.type === "edited") && showsWho(property)) {
        return (
          <Select
            label={label}
            hideLabel
            value={text}
            options={[
              { value: "", label: t("tables.filter.chooseMember") },
              ...members.map((member) => ({
                value: member.user_id,
                label: member.display_name ?? member.email ?? member.user_id,
              })),
            ]}
            onChange={onChange}
          />
        );
      }
      return (
        <DatePicker
          label={label}
          hideLabel
          value={text === "" ? undefined : text.slice(0, 10)}
          onChange={(day) => {
            onChange(day ?? "");
          }}
        />
      );
    }
    case "person": {
      return (
        <Select
          label={label}
          hideLabel
          value={text}
          options={[
            { value: "", label: t("tables.filter.chooseMember") },
            ...members.map((member) => ({
              value: member.user_id,
              label: member.display_name ?? member.email ?? member.user_id,
            })),
          ]}
          onChange={onChange}
        />
      );
    }
    case "relation": {
      const table =
        isRecord(property.config) && typeof property.config["table"] === "string"
          ? property.config["table"]
          : "";
      return (
        <RowPicker
          ctx={ctx}
          table={table}
          selected={text === "" ? [] : [text]}
          onChange={(ids) => {
            onChange(ids.at(-1) ?? "");
          }}
        />
      );
    }
    case "campbooks_link": {
      const chosen = isRecord(condition.value) ? linksOf([condition.value]) : [];
      return (
        <div className="flex flex-col gap-1">
          {chosen.map((link) => (
            <LinkChip key={`${link.type}:${link.id}`} ctx={ctx} link={link} />
          ))}
          <LinkPicker
            ctx={ctx}
            property={property}
            pages={pages}
            selected={chosen}
            onChange={(links) => {
              const last = links.at(-1);
              onChange(last === undefined ? undefined : { type: last.type, id: last.id });
            }}
          />
        </div>
      );
    }
    default: {
      return (
        <TextField
          label={label}
          hideLabel
          value={text}
          inputMode={
            property.type === "number" || property.type === "unique_id" ? "decimal" : undefined
          }
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      );
    }
  }
}

export interface FilterBuilderProps {
  ctx: CellContext;
  properties: readonly PropertyView[];
  filter: FilterNode | undefined;
  pages: readonly PageSummary[];
  /** A column to start a new condition on (from a column's menu). */
  focus?: string | undefined;
  onApply: (filter?: FilterNode) => void;
  onClose: () => void;
}

/**
 * The filter of a view as conditions: a property, how to compare it, and
 * with what, all of them or any. Nothing changes the view until Apply.
 */
export function FilterBuilder({
  ctx,
  properties,
  filter,
  pages,
  focus,
  onApply,
  onClose,
}: FilterBuilderProps) {
  const { t } = useTranslation("pages");
  const usable = properties.filter((property) => filterable(property));
  const [draft, setDraft] = useState<FlatFilter>(() => {
    const flat = flatFilter(filter);
    const first = usable.find((property) => property.id === focus);
    if (first === undefined || flat.conditions.some((c) => c.property === first.id)) {
      return flat;
    }
    const operator = operatorsFor(first)[0] ?? "is";
    return {
      ...flat,
      conditions: [
        ...flat.conditions,
        { property: first.id, operator, value: startingValue(first, operator) },
      ],
    };
  });

  const change = (index: number, next: Partial<Condition> & { property?: string }) => {
    setDraft((current) => ({
      ...current,
      conditions: current.conditions.map((condition, at) => {
        if (at !== index) {
          return condition;
        }
        const merged = { ...condition, ...next };
        const property = usable.find((candidate) => candidate.id === merged.property);
        if (property === undefined) {
          return merged;
        }
        // A new property, or an operator that needs another kind of value, starts afresh.
        if (next.property !== undefined && next.property !== condition.property) {
          const operator = operatorsFor(property)[0] ?? "is";
          return { property: property.id, operator, value: startingValue(property, operator) };
        }
        if (next.operator !== undefined && next.operator !== condition.operator) {
          return {
            ...merged,
            value:
              next.operator === "is_any_of" ||
              condition.operator === "is_any_of" ||
              !takesValue(next.operator)
                ? startingValue(property, next.operator)
                : condition.value,
          };
        }
        return merged;
      }),
    }));
  };

  const add = () => {
    const first = usable[0];
    if (first === undefined) {
      return;
    }
    const operator = operatorsFor(first)[0] ?? "is";
    setDraft((current) => ({
      ...current,
      conditions: [
        ...current.conditions,
        { property: first.id, operator, value: startingValue(first, operator) },
      ],
    }));
  };

  const ready = draft.conditions.filter((condition) => conditionReady(condition));

  return (
    <div className="flex flex-col gap-3 p-3">
      {draft.nested ? <p>{t("tables.filter.nested")}</p> : null}
      {draft.conditions.length === 0 && !draft.nested ? <p>{t("tables.filter.none")}</p> : null}
      {draft.conditions.length > 1 ? (
        <Segmented
          label={t("tables.filter.mode")}
          value={draft.mode}
          options={[
            { value: "and", label: t("tables.filter.all") },
            { value: "or", label: t("tables.filter.any") },
          ]}
          onChange={(mode) => {
            setDraft((current) => ({ ...current, mode }));
          }}
        />
      ) : null}
      {draft.nested
        ? null
        : draft.conditions.map((condition, index) => {
            const property = usable.find((candidate) => candidate.id === condition.property);
            return (
              <div
                key={index}
                className="flex flex-col gap-2"
                role="group"
                aria-label={t("tables.filter.condition", { number: index + 1 })}
              >
                <div className="flex items-end gap-2">
                  <Select
                    label={t("tables.filter.property")}
                    hideLabel
                    value={condition.property}
                    options={usable.map((candidate) => ({
                      value: candidate.id,
                      label: candidate.name,
                    }))}
                    onChange={(value) => {
                      change(index, { property: value });
                    }}
                  />
                  <IconButton
                    icon="close"
                    label={t("tables.filter.remove", { number: index + 1 })}
                    onClick={() => {
                      setDraft((current) => ({
                        ...current,
                        conditions: current.conditions.filter((_, at) => at !== index),
                      }));
                    }}
                  />
                </div>
                {property === undefined ? null : (
                  <Select<Operator>
                    label={t("tables.filter.operator")}
                    hideLabel
                    value={condition.operator}
                    options={operatorsFor(property).map((operator) => ({
                      value: operator,
                      label: t(`tables.operator.${operator}`),
                    }))}
                    onChange={(operator) => {
                      change(index, { operator });
                    }}
                  />
                )}
                {property === undefined || !takesValue(condition.operator) ? null : (
                  <ValueInput
                    ctx={ctx}
                    property={property}
                    condition={condition}
                    members={ctx.members}
                    pages={pages}
                    onChange={(value) => {
                      change(index, { value });
                    }}
                  />
                )}
              </div>
            );
          })}
      <div className="flex flex-wrap gap-2">
        {draft.nested ? null : (
          <Button variant="ghost" icon="plus" onClick={add}>
            {t("tables.filter.add")}
          </Button>
        )}
        <Button
          variant="primary"
          onClick={() => {
            onApply(draft.nested ? filter : filterOf({ mode: draft.mode, conditions: ready }));
            onClose();
          }}
        >
          {t("tables.filter.apply")}
        </Button>
        {draft.conditions.length > 0 || draft.nested ? (
          <Button
            variant="ghost"
            onClick={() => {
              onApply();
              onClose();
            }}
          >
            {t("tables.filter.clear")}
          </Button>
        ) : null}
      </div>
    </div>
  );
}

// --------------------------------------------------------------------- sorts

export interface SortBuilderProps {
  properties: readonly PropertyView[];
  sorts: readonly SortSpec[];
  onApply: (sorts: SortSpec[]) => void;
  onClose: () => void;
}

/** The sort of a view: up to five properties, each ascending or descending. */
export function SortBuilder({ properties, sorts, onApply, onClose }: SortBuilderProps) {
  const { t } = useTranslation("pages");
  const usable = properties.filter((property) => sortable(property));
  const [draft, setDraft] = useState<SortSpec[]>([...sorts]);
  const free = usable.filter((property) => !draft.some((sort) => sort.property === property.id));
  return (
    <div className="flex flex-col gap-3 p-3">
      {draft.length === 0 ? <p>{t("tables.sort.none")}</p> : null}
      {draft.map((sort, index) => (
        <div
          key={sort.property}
          className="flex items-end gap-2"
          role="group"
          aria-label={t("tables.sort.level", { number: index + 1 })}
        >
          <Select
            label={t("tables.filter.property")}
            hideLabel
            value={sort.property}
            options={usable
              .filter(
                (property) =>
                  property.id === sort.property ||
                  !draft.some((other) => other.property === property.id),
              )
              .map((property) => ({ value: property.id, label: property.name }))}
            onChange={(value) => {
              setDraft((current) =>
                current.map((item, at) => (at === index ? { ...item, property: value } : item)),
              );
            }}
          />
          <Select<SortDirection>
            label={t("tables.sort.direction")}
            hideLabel
            value={sort.direction}
            options={[
              { value: "asc", label: t("tables.sort.asc") },
              { value: "desc", label: t("tables.sort.desc") },
            ]}
            onChange={(direction) => {
              setDraft((current) =>
                current.map((item, at) => (at === index ? { ...item, direction } : item)),
              );
            }}
          />
          <IconButton
            icon="close"
            label={t("tables.sort.remove", { number: index + 1 })}
            onClick={() => {
              setDraft((current) => current.filter((_, at) => at !== index));
            }}
          />
        </div>
      ))}
      <div className="flex flex-wrap gap-2">
        {free.length === 0 || draft.length >= 5 ? null : (
          <Button
            variant="ghost"
            icon="plus"
            onClick={() => {
              const first = free[0];
              if (first !== undefined) {
                setDraft((current) => [...current, { property: first.id, direction: "asc" }]);
              }
            }}
          >
            {t("tables.sort.add")}
          </Button>
        )}
        <Button
          variant="primary"
          onClick={() => {
            onApply(draft);
            onClose();
          }}
        >
          {t("tables.filter.apply")}
        </Button>
      </div>
    </div>
  );
}
