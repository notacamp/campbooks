import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  IconButton,
  OptionTag,
  Picker,
  Select,
  TextField,
  type PickerItem,
} from "~/lib/ui";

import { focusOnMount } from "../focus";
import { colorOf, nextColor, optionsOf } from "../model";
import {
  OPTION_COLORS,
  type ChoiceOption,
  type OptionColor,
  type OptionDraft,
  type PropertyView,
} from "../types";

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

export interface OptionPickerProps {
  property: PropertyView;
  /** The option ids that are on. */
  selected: readonly string[];
  /** Several at once. */
  multiple: boolean;
  /** An option was picked (or taken off, for several). */
  onToggle: (id: string) => void;
  /** Clear the cell. */
  onClear: () => void;
  /** Write a changed list of options; answers the property as the server has it. */
  onOptions: (
    next: readonly OptionDraft[],
    previous: readonly OptionDraft[],
  ) => Promise<PropertyView | undefined>;
  /** The member may not change the options (a reader never gets here). */
  canEditOptions: boolean;
}

/**
 * The options of a select, multi-select or status property: search them by
 * name, pick, make a new one from what was typed, rename or recolour one, or
 * delete it. Renaming changes every row that holds it; nothing is rewritten.
 */
export function OptionPicker({
  property,
  selected,
  multiple,
  onToggle,
  onClear,
  onOptions,
  canEditOptions,
}: OptionPickerProps) {
  const { t } = useTranslation("pages");
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<string | undefined>();
  const options = optionsOf(property);
  const typed = query.trim();
  const shown = options.filter((option) => fold(option.name).includes(fold(typed)));
  const exact = options.some((option) => fold(option.name) === fold(typed));

  if (editing !== undefined) {
    const option = options.find((candidate) => candidate.id === editing);
    if (option !== undefined) {
      return (
        <OptionEditor
          option={option}
          onBack={() => {
            setEditing(undefined);
          }}
          onSave={async (changed) => {
            await onOptions(
              options.map((candidate) => (candidate.id === changed.id ? changed : candidate)),
              options,
            );
            setEditing(undefined);
          }}
          onDelete={async () => {
            await onOptions(
              options.filter((candidate) => candidate.id !== option.id),
              options,
            );
            setEditing(undefined);
          }}
        />
      );
    }
  }

  const items: PickerItem[] = shown.map((option) => ({
    id: option.id,
    text: option.name,
    selected: selected.includes(option.id),
    label: <OptionTag color={colorOf(option.color)}>{option.name}</OptionTag>,
    ...(option.group === undefined ? {} : { hint: t(`tables.status.${option.group}`) }),
    end: canEditOptions ? (
      <IconButton
        icon="edit"
        label={t("tables.picker.editOption", { name: option.name })}
        onClick={() => {
          setEditing(option.id);
        }}
      />
    ) : undefined,
  }));

  return (
    <Picker
      label={property.name}
      searchLabel={t("tables.picker.searchOptions")}
      placeholder={t("tables.picker.searchOptions")}
      query={query}
      onQuery={setQuery}
      items={items}
      multiple={multiple}
      empty={t("tables.picker.noOptions")}
      onPick={onToggle}
      create={
        canEditOptions && typed !== "" && !exact
          ? {
              label: t("tables.picker.createOption", { name: typed }),
              onCreate: () => {
                const group = property.type === "status" ? ("todo" as const) : undefined;
                const made: OptionDraft = {
                  name: typed.slice(0, 100),
                  color: nextColor(options),
                  ...(group === undefined ? {} : { group }),
                };
                void onOptions([...options, made], options).then((updated) => {
                  const created =
                    updated === undefined
                      ? undefined
                      : optionsOf(updated).find((option) => option.name === made.name);
                  if (created !== undefined) {
                    setQuery("");
                    onToggle(created.id);
                  }
                });
              },
            }
          : undefined
      }
      footer={
        <Button variant="ghost" onClick={onClear}>
          {t("tables.picker.clear")}
        </Button>
      }
    />
  );
}

interface OptionEditorProps {
  option: ChoiceOption;
  onBack: () => void;
  onSave: (option: ChoiceOption) => Promise<void>;
  onDelete: () => Promise<void>;
}

/** One option: its name and its colour, and Delete. */
function OptionEditor({ option, onBack, onSave, onDelete }: OptionEditorProps) {
  const { t } = useTranslation("pages");
  const [name, setName] = useState(option.name);
  const [color, setColor] = useState<OptionColor>(colorOf(option.color));
  const valid = name.trim() !== "" && name.trim().length <= 100;
  return (
    <div className="flex flex-col gap-3 p-3">
      <TextField
        label={t("tables.picker.optionName")}
        value={name}
        maxLength={100}
        ref={focusOnMount}
        onChange={(event) => {
          setName(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && valid) {
            event.preventDefault();
            void onSave({ ...option, name: name.trim(), color });
          }
        }}
      />
      <Select
        label={t("tables.picker.optionColor")}
        value={color}
        options={OPTION_COLORS.map((candidate) => ({
          value: candidate,
          label: t(`tables.colors.${candidate}`),
        }))}
        onChange={setColor}
      />
      <OptionTag color={color}>
        {name.trim() === "" ? t("tables.picker.optionName") : name}
      </OptionTag>
      <div className="flex flex-wrap gap-2">
        <Button
          variant="primary"
          disabled={!valid}
          onClick={() => {
            void onSave({ ...option, name: name.trim(), color });
          }}
        >
          {t("tables.picker.saveOption")}
        </Button>
        <Button variant="ghost" onClick={onBack}>
          {t("tables.picker.back")}
        </Button>
        <Button
          variant="ghost"
          icon="trash"
          onClick={() => {
            void onDelete();
          }}
        >
          {t("tables.picker.deleteOption")}
        </Button>
      </div>
    </div>
  );
}
