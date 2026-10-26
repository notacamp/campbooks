import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { CellButton, CellInput, Popover } from "~/lib/ui";

import type { PageSummary, UploadedFile } from "../../types";
import {
  decimalOf,
  idsOf,
  isRecord,
  linksOf,
  plainText,
  runsOf,
  safeHref,
  type FileRef,
} from "../model";
import type { PropertyView, RowView, OptionDraft } from "../types";

import { CellDisplay } from "./CellDisplay";
import type { CellContext } from "./context";
import { OptionPicker } from "./OptionPicker";
import { DateEditor, FilesEditor, LinkPicker, PersonPicker, RowPicker } from "./Pickers";

export interface CellEditorProps {
  ctx: CellContext;
  property: PropertyView;
  row: RowView;
  value: unknown;
  /** The character that started the edit: it replaces what the cell held. */
  seed?: string | undefined;
  /** The new value (`null` clears the cell). */
  onCommit: (value: unknown) => void;
  /** Close without writing. */
  onCancel: () => void;
  /** Write a changed list of options. */
  onOptions: (
    next: readonly OptionDraft[],
    previous: readonly OptionDraft[],
  ) => Promise<PropertyView | undefined>;
  upload: (row: string, file: File) => Promise<UploadedFile>;
  pages: readonly PageSummary[];
  canEditOptions: boolean;
}

type TextKind = "title" | "text" | "number" | "url" | "email" | "phone";

const TEXT_KINDS = new Set(["title", "text", "number", "url", "email", "phone"]);

function isTextKind(type: string): type is TextKind {
  return TEXT_KINDS.has(type);
}

/** What the editor starts with. */
function initialText(type: TextKind, row: RowView, value: unknown): string {
  switch (type) {
    case "title": {
      return row.title;
    }
    case "text": {
      return plainText(value);
    }
    default: {
      return typeof value === "string" ? value : "";
    }
  }
}

/** The text read as the type's value, or `undefined` when it is not one. `null` clears. */
function parseText(type: TextKind, input: string): { value: unknown } | undefined {
  const trimmed = input.trim();
  switch (type) {
    case "title": {
      return { value: trimmed };
    }
    case "text": {
      return { value: runsOf(trimmed) };
    }
    case "number": {
      if (trimmed === "") {
        return { value: null };
      }
      const decimal = decimalOf(trimmed);
      return decimal === undefined ? undefined : { value: decimal };
    }
    case "url": {
      if (trimmed === "") {
        return { value: null };
      }
      return /^https?:\/\//iu.test(trimmed) && safeHref(trimmed) !== undefined
        ? { value: trimmed }
        : undefined;
    }
    case "email": {
      if (trimmed === "") {
        return { value: null };
      }
      return /^[^\s@,<]+@[^\s@,<]+\.[^\s@,<]+$/u.test(trimmed) ? { value: trimmed } : undefined;
    }
    case "phone": {
      return { value: trimmed === "" ? null : trimmed };
    }
  }
}

function TextEditor({
  property,
  row,
  value,
  seed,
  onCommit,
  onCancel,
}: CellEditorProps & { property: PropertyView & { type: TextKind } }) {
  const { t } = useTranslation("pages");
  const type = property.type;
  const was = initialText(type, row, value);
  const [draft, setDraft] = useState(seed ?? was);
  const done = useRef(false);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const input = field.current;
    if (input !== null) {
      input.focus();
      const end = input.value.length;
      input.setSelectionRange(end, end);
    }
  }, []);
  const parsed = parseText(type, draft);
  const finish = (commit: boolean) => {
    if (done.current) {
      return;
    }
    // What does not read as a value is left as it was.
    if (!commit || parsed === undefined || draft.trim() === was.trim()) {
      done.current = true;
      onCancel();
      return;
    }
    done.current = true;
    onCommit(parsed.value);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    switch (event.key) {
      case "Enter": {
        event.preventDefault();
        if (parsed !== undefined) {
          finish(true);
        }
        break;
      }
      case "Escape": {
        event.preventDefault();
        event.stopPropagation();
        finish(false);
        break;
      }
      case "Tab": {
        finish(true);
        break;
      }
      default:
    }
  };
  return (
    <CellInput
      ref={field}
      type="text"
      inputMode={type === "number" ? "decimal" : undefined}
      aria-label={`${property.name} — ${row.title === "" ? t("untitled") : row.title}`}
      aria-invalid={parsed === undefined ? true : undefined}
      value={draft}
      maxLength={type === "title" ? 200 : 2000}
      onChange={(event) => {
        setDraft(event.target.value);
      }}
      onKeyDown={onKeyDown}
      onBlur={() => {
        finish(true);
      }}
    />
  );
}

/** A picker in a popover over the cell, closed (and its choice written) with Escape or a click outside. */
function PopoverEditor({
  ctx,
  property,
  row,
  value,
  onClose,
  children,
}: {
  ctx: CellContext;
  property: PropertyView;
  row: RowView;
  value: unknown;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <Popover
      open
      onOpenChange={(open) => {
        if (!open) {
          onClose();
        }
      }}
      label={property.name}
      placement="bottom-start"
      layer="top"
      renderTrigger={(trigger) => (
        <CellButton {...trigger} aria-label={property.name}>
          <CellDisplay ctx={ctx} property={property} row={row} value={value} />
        </CellButton>
      )}
    >
      {children}
    </Popover>
  );
}

/** A choice kept locally while the popover is open, written when it closes (if it changed). */
function useDraft<TValue>(initial: TValue, same: (a: TValue, b: TValue) => boolean) {
  const [draft, setDraft] = useState(initial);
  const latest = useRef(draft);
  const update = (next: TValue) => {
    latest.current = next;
    setDraft(next);
  };
  return { draft, update, changed: () => !same(latest.current, initial), latest };
}

const sameIds = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && a.every((id, index) => id === b[index]);

function ChoiceEditor(props: CellEditorProps) {
  const { property, value, onCommit, onCancel, onOptions, canEditOptions } = props;
  const multiple = property.type === "multi_select";
  const chosen = useDraft(idsOf(value), sameIds);
  const close = () => {
    if (multiple && chosen.changed()) {
      onCommit(chosen.latest.current.length === 0 ? null : chosen.latest.current);
    } else {
      onCancel();
    }
  };
  return (
    <PopoverEditor {...props} onClose={close}>
      <OptionPicker
        property={property}
        selected={chosen.draft}
        multiple={multiple}
        canEditOptions={canEditOptions}
        onOptions={onOptions}
        onClear={() => {
          onCommit(null);
        }}
        onToggle={(id) => {
          if (multiple) {
            chosen.update(
              chosen.latest.current.includes(id)
                ? chosen.latest.current.filter((item) => item !== id)
                : [...chosen.latest.current, id],
            );
          } else {
            onCommit(id);
          }
        }}
      />
    </PopoverEditor>
  );
}

function PeopleEditor(props: CellEditorProps) {
  const { ctx, value, onCommit, onCancel } = props;
  const chosen = useDraft(idsOf(value), sameIds);
  return (
    <PopoverEditor
      {...props}
      onClose={() => {
        if (chosen.changed()) {
          onCommit(chosen.latest.current.length === 0 ? null : chosen.latest.current);
        } else {
          onCancel();
        }
      }}
    >
      <PersonPicker ctx={ctx} selected={chosen.draft} onChange={chosen.update} />
    </PopoverEditor>
  );
}

function RelationEditor(props: CellEditorProps) {
  const { ctx, property, value, onCommit, onCancel } = props;
  const chosen = useDraft(idsOf(value), sameIds);
  const table =
    isRecord(property.config) && typeof property.config["table"] === "string"
      ? property.config["table"]
      : "";
  return (
    <PopoverEditor
      {...props}
      onClose={() => {
        if (chosen.changed()) {
          onCommit(chosen.latest.current.length === 0 ? null : chosen.latest.current);
        } else {
          onCancel();
        }
      }}
    >
      <RowPicker ctx={ctx} table={table} selected={chosen.draft} onChange={chosen.update} />
    </PopoverEditor>
  );
}

function LinksEditor(props: CellEditorProps) {
  const { ctx, property, value, pages, onCommit, onCancel } = props;
  const chosen = useDraft(
    linksOf(value),
    (a, b) =>
      a.length === b.length &&
      a.every((link, i) => link.id === b[i]?.id && link.type === b[i].type),
  );
  return (
    <PopoverEditor
      {...props}
      onClose={() => {
        if (chosen.changed()) {
          onCommit(chosen.latest.current.length === 0 ? null : chosen.latest.current);
        } else {
          onCancel();
        }
      }}
    >
      <LinkPicker
        ctx={ctx}
        property={property}
        pages={pages}
        selected={chosen.draft}
        onChange={chosen.update}
      />
    </PopoverEditor>
  );
}

function DateCellEditor(props: CellEditorProps) {
  const { property, value, onCommit, onCancel } = props;
  const latest = useRef<Record<string, string> | null | undefined>(undefined);
  const withTime = isRecord(property.config) && property.config["time"] === true;
  const close = () => {
    if (latest.current === undefined) {
      onCancel();
    } else {
      onCommit(latest.current);
    }
  };
  return (
    <PopoverEditor {...props} onClose={close}>
      <DateEditor
        value={value}
        withTime={withTime}
        onChange={(next) => {
          latest.current = next;
        }}
        onDone={close}
      />
    </PopoverEditor>
  );
}

function FilesCellEditor(props: CellEditorProps) {
  const { value, row, onCommit, onCancel, upload } = props;
  const latest = useRef<FileRef[] | undefined>(undefined);
  const close = () => {
    if (latest.current === undefined) {
      onCancel();
    } else {
      onCommit(latest.current.length === 0 ? null : latest.current);
    }
  };
  return (
    <PopoverEditor {...props} onClose={close}>
      <FilesEditor
        value={value}
        upload={(file) => upload(row.id, file)}
        onChange={(next) => {
          latest.current = next;
        }}
        onDone={close}
      />
    </PopoverEditor>
  );
}

/** The editor for a cell of this type: a field in the cell, or a picker over it. */
export function CellEditor(props: CellEditorProps) {
  const type = props.property.type;
  if (isTextKind(type)) {
    return <TextEditor {...props} property={props.property as PropertyView & { type: TextKind }} />;
  }
  switch (type) {
    case "select":
    case "status":
    case "multi_select": {
      return <ChoiceEditor {...props} />;
    }
    case "person": {
      return <PeopleEditor {...props} />;
    }
    case "relation": {
      return <RelationEditor {...props} />;
    }
    case "campbooks_link": {
      return <LinksEditor {...props} />;
    }
    case "date": {
      return <DateCellEditor {...props} />;
    }
    case "files": {
      return <FilesCellEditor {...props} />;
    }
    default: {
      return null;
    }
  }
}
