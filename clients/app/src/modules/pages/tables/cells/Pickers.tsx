import { useQuery } from "@tanstack/react-query";
import { useDeferredValue, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Avatar,
  Button,
  Checkbox,
  DatePicker,
  Picker,
  TimePicker,
  type PickerItem,
} from "~/lib/ui";

import { searchMentions, type MentionCandidate } from "../../api/mention-search";
import type { MentionType, PageSummary, UploadedFile } from "../../types";
import { useTable, useTableCalls } from "../api";
import {
  dateJson,
  dateOf,
  dayOf,
  filesOf,
  joinWhen,
  timeOf,
  type DateValue,
  type FileRef,
  type LinkRef,
} from "../model";
import type { PropertyView } from "../types";

import { memberName, type CellContext } from "./context";
import { LinkChip } from "./CellDisplay";

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

const toggled = (list: readonly string[], id: string): string[] =>
  list.includes(id) ? list.filter((item) => item !== id) : [...list, id];

// ------------------------------------------------------------------- people

export interface PersonPickerProps {
  ctx: CellContext;
  selected: readonly string[];
  onChange: (ids: string[]) => void;
}

/** Teammates, found by name: never by typing an address. */
export function PersonPicker({ ctx, selected, onChange }: PersonPickerProps) {
  const { t } = useTranslation("pages");
  const [query, setQuery] = useState("");
  const needle = fold(query.trim());
  const items: PickerItem[] = ctx.members
    .filter(
      (member) =>
        needle === "" ||
        fold(member.display_name ?? "").includes(needle) ||
        fold(member.email ?? "").includes(needle),
    )
    .map((member) => {
      const name = memberName(ctx.members, member.user_id) ?? member.user_id;
      return {
        id: member.user_id,
        text: name,
        selected: selected.includes(member.user_id),
        label: (
          <>
            <Avatar name={name} size="sm" />
            <span>{name}</span>
          </>
        ),
      };
    });
  return (
    <Picker
      label={t("tables.picker.people")}
      searchLabel={t("tables.picker.searchPeople")}
      placeholder={t("tables.picker.searchPeople")}
      query={query}
      onQuery={setQuery}
      items={items}
      multiple
      empty={t("tables.picker.noPeople")}
      onPick={(id) => {
        onChange(toggled(selected, id));
      }}
    />
  );
}

// --------------------------------------------------------------------- rows

export interface RowPickerProps {
  ctx: CellContext;
  /** The table the relation points at. */
  table: string;
  selected: readonly string[];
  onChange: (ids: string[]) => void;
}

/** Rows of the related table, found by title. A table the viewer cannot read offers none. */
export function RowPicker({ ctx, table, selected, onChange }: RowPickerProps) {
  const { t } = useTranslation("pages");
  const [query, setQuery] = useState("");
  const typed = useDeferredValue(query.trim());
  const calls = useTableCalls(ctx.workspaceId);
  const schema = useTable(ctx.workspaceId, table);
  const title = schema.data?.properties.find((property) => property.type === "title");
  const found = useQuery({
    queryKey: ["pages", "tables", "row-search", ctx.workspaceId, table, typed],
    enabled: title !== undefined,
    queryFn: ({ signal }) =>
      calls.query(
        table,
        {
          limit: 20,
          ...(typed === ""
            ? {}
            : {
                filter: {
                  and: [{ property: title?.id ?? "", operator: "contains", value: typed }],
                },
              }),
        },
        signal,
      ),
  });
  if (schema.isError && errorStatus(schema.error) === 404) {
    return <p className="p-3">{t("mention.closed")}</p>;
  }
  const rows = found.data?.rows ?? [];
  const items: PickerItem[] = rows.map((row) => ({
    id: row.id,
    text: row.title === "" ? t("untitled") : row.title,
    selected: selected.includes(row.id),
    label: <span>{row.title === "" ? t("untitled") : row.title}</span>,
  }));
  // What is already chosen stays on the list even when the search does not find it.
  for (const id of selected) {
    if (!items.some((item) => item.id === id)) {
      items.unshift({
        id,
        text: id,
        selected: true,
        label: <span>{t("tables.picker.chosenRow")}</span>,
      });
    }
  }
  return (
    <Picker
      label={t("tables.picker.rows")}
      searchLabel={t("tables.picker.searchRows")}
      placeholder={t("tables.picker.searchRows")}
      query={query}
      onQuery={setQuery}
      items={items}
      multiple
      empty={t("tables.picker.noRows")}
      {...(found.isPending ? { loading: t("tables.picker.loading") } : {})}
      onPick={(id) => {
        onChange(toggled(selected, id));
      }}
    />
  );
}

// -------------------------------------------------------------------- links

const KIND_OF = {
  "workspaces.member": "person",
  person: "person",
  "conversations.conversation": "thread",
  "tasks.task": "task",
  "schedule.event": "event",
  "pages.page": "page",
} as const satisfies Record<MentionType, string>;

const keyOf = (link: LinkRef) => `${link.type}\u0000${link.id}`;

export interface LinkPickerProps {
  ctx: CellContext;
  property: PropertyView;
  pages: readonly PageSummary[];
  selected: readonly LinkRef[];
  onChange: (links: LinkRef[]) => void;
}

/** Threads, tasks, events, people and pages, found by name (the viewer's own search). */
export function LinkPicker({ ctx, property, pages, selected, onChange }: LinkPickerProps) {
  const { t } = useTranslation("pages");
  const [query, setQuery] = useState("");
  const typed = useDeferredValue(query.trim());
  const allowed =
    typeof property.config === "object" &&
    property.config !== null &&
    Array.isArray((property.config as { types?: unknown }).types)
      ? (property.config as { types: unknown[] }).types.filter(
          (type): type is string => typeof type === "string",
        )
      : [];
  const found = useQuery({
    queryKey: ["pages", "tables", "link-search", ctx.workspaceId, typed],
    queryFn: ({ signal }) => searchMentions(ctx.workspaceId, typed, pages, signal),
  });
  const candidates: MentionCandidate[] = (found.data ?? [])
    .flatMap((group) => group.candidates)
    .filter((candidate) => allowed.length === 0 || allowed.includes(candidate.type));
  const chosen = new Set(selected.map((link) => keyOf(link)));
  const items: PickerItem[] = [
    ...selected.map((link) => ({
      id: keyOf(link),
      text: link.id,
      selected: true,
      label: <LinkChip ctx={ctx} link={link} />,
    })),
    ...candidates
      .filter((candidate) => !chosen.has(keyOf(candidate)))
      .map((candidate) => ({
        id: keyOf(candidate),
        text: candidate.label,
        selected: false,
        label: <span>{candidate.label}</span>,
        hint: candidate.detail ?? t(`embed.${KIND_OF[candidate.type]}`),
      })),
  ];
  return (
    <Picker
      label={t("tables.picker.links")}
      searchLabel={t("tables.picker.searchLinks")}
      placeholder={t("tables.picker.searchLinks")}
      query={query}
      onQuery={setQuery}
      items={items}
      multiple
      empty={t("tables.picker.noLinks")}
      {...(found.isPending ? { loading: t("tables.picker.loading") } : {})}
      onPick={(id) => {
        if (chosen.has(id)) {
          onChange(selected.filter((link) => keyOf(link) !== id));
          return;
        }
        const candidate = candidates.find((item) => keyOf(item) === id);
        if (candidate !== undefined) {
          onChange([...selected, { type: candidate.type, id: candidate.id }]);
        }
      }}
    />
  );
}

// -------------------------------------------------------------------- dates

export interface DateEditorProps {
  value: unknown;
  /** Whether the property keeps a time of day. */
  withTime: boolean;
  onChange: (value: Record<string, string> | null) => void;
  onDone: () => void;
}

/** A day (and a time), or a range of them, picked from a month grid. */
export function DateEditor({ value, withTime, onChange, onDone }: DateEditorProps) {
  const { t } = useTranslation("pages");
  const initial = dateOf(value);
  const [start, setStart] = useState<string | undefined>(
    initial === undefined ? undefined : dayOf(initial.start),
  );
  const [startTime, setStartTime] = useState<string | undefined>(
    initial === undefined ? undefined : timeOf(initial.start),
  );
  const [end, setEnd] = useState<string | undefined>(
    initial?.end === undefined ? undefined : dayOf(initial.end),
  );
  const [endTime, setEndTime] = useState<string | undefined>(
    initial?.end === undefined ? undefined : timeOf(initial.end),
  );
  const [ranged, setRanged] = useState(initial?.end !== undefined);

  const settle = (
    nextStart: string | undefined,
    nextStartTime: string | undefined,
    nextEnd: string | undefined,
    nextEndTime: string | undefined,
    isRange: boolean,
  ) => {
    if (nextStart === undefined) {
      onChange(null);
      return;
    }
    const time = withTime ? (nextStartTime ?? "09:00") : undefined;
    const result: DateValue = { start: joinWhen(nextStart, time) };
    if (isRange && nextEnd !== undefined && nextEnd >= nextStart) {
      result.end = joinWhen(nextEnd, withTime ? (nextEndTime ?? time) : undefined);
    }
    onChange(dateJson(result));
  };

  return (
    <div className="flex flex-col gap-3 p-3">
      <DatePicker
        label={t("tables.date.start")}
        hideLabel
        inline
        clearable
        value={start}
        onChange={(day) => {
          setStart(day);
          if (day === undefined) {
            setEnd(undefined);
            setRanged(false);
          }
          settle(day, startTime, end, endTime, ranged);
        }}
      />
      {withTime ? (
        <TimePicker
          label={t("tables.date.startTime")}
          value={startTime ?? (start === undefined ? undefined : "09:00")}
          onChange={(time) => {
            setStartTime(time);
            settle(start, time, end, endTime, ranged);
          }}
        />
      ) : null}
      <Checkbox
        label={t("tables.date.range")}
        checked={ranged}
        disabled={start === undefined}
        onChange={(event) => {
          setRanged(event.target.checked);
          const nextEnd = event.target.checked ? (end ?? start) : undefined;
          setEnd(nextEnd);
          settle(start, startTime, nextEnd, endTime, event.target.checked);
        }}
      />
      {ranged ? (
        <>
          <DatePicker
            label={t("tables.date.end")}
            hideLabel
            inline
            value={end}
            min={start}
            onChange={(day) => {
              setEnd(day);
              settle(start, startTime, day, endTime, true);
            }}
          />
          {withTime ? (
            <TimePicker
              label={t("tables.date.endTime")}
              value={endTime ?? startTime ?? "09:00"}
              onChange={(time) => {
                setEndTime(time);
                settle(start, startTime, end, time, true);
              }}
            />
          ) : null}
        </>
      ) : null}
      <div className="flex gap-2">
        <Button variant="primary" onClick={onDone}>
          {t("tables.picker.done")}
        </Button>
      </div>
    </div>
  );
}

// -------------------------------------------------------------------- files

export interface FilesEditorProps {
  value: unknown;
  onChange: (files: FileRef[]) => void;
  /** Store a file with the row's page. */
  upload: (file: File) => Promise<UploadedFile>;
  onDone: () => void;
}

/** The files of a cell: add one from the device, or take one off. */
export function FilesEditor({ value, onChange, upload, onDone }: FilesEditorProps) {
  const { t } = useTranslation("pages");
  const input = useRef<HTMLInputElement>(null);
  const [files, setFiles] = useState<FileRef[]>(filesOf(value));
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);

  const add = async (file: File) => {
    setBusy(true);
    setFailed(false);
    try {
      const stored = await upload(file);
      const next = [
        ...files,
        { hash: stored.blob, name: file.name, size: stored.size, type: stored.content_type },
      ];
      setFiles(next);
      onChange(next);
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-2 p-3">
      <ul className="m-0 flex list-none flex-col gap-1 p-0" aria-label={t("tables.files.list")}>
        {files.map((file) => (
          <li key={file.hash} className="flex items-center gap-2">
            <span className="min-w-0 flex-1 break-words">{file.name}</span>
            <Button
              variant="ghost"
              icon="close"
              aria-label={t("tables.files.remove", { name: file.name })}
              onClick={() => {
                const next = files.filter((candidate) => candidate.hash !== file.hash);
                setFiles(next);
                onChange(next);
              }}
            >
              {t("tables.files.removeShort")}
            </Button>
          </li>
        ))}
      </ul>
      <input
        ref={input}
        type="file"
        hidden
        aria-label={t("tables.files.choose")}
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = "";
          if (file !== undefined) {
            void add(file);
          }
        }}
      />
      {failed ? <p role="alert">{t("tables.files.failed")}</p> : null}
      <div className="flex gap-2">
        <Button
          icon="attachment"
          disabled={busy || files.length >= 20}
          onClick={() => {
            input.current?.click();
          }}
        >
          {busy ? t("tables.files.uploading") : t("tables.files.add")}
        </Button>
        <Button variant="primary" onClick={onDone}>
          {t("tables.picker.done")}
        </Button>
      </div>
    </div>
  );
}
