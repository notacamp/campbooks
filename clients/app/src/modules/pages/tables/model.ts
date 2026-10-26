import {
  OPTION_COLORS,
  type ChoiceOption,
  type ColumnSpec,
  type Condition,
  type FilterNode,
  type Operator,
  type OptionColor,
  type PropertyType,
  type PropertyView,
  type SortSpec,
  type SummaryKind,
  type SummarySpec,
  type TableViewView,
  type ViewConfig,
} from "./types";

/** What the pure parts of a table screen know: view configs, types, operators, values. */

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

const text = (value: unknown): string | undefined =>
  typeof value === "string" ? value : undefined;

// ---------------------------------------------------------------- view config

const SORT_DIRECTIONS = ["asc", "desc"] as const;
const SUMMARY_KINDS = ["count", "sum", "average", "min", "max", "percent_checked"] as const;
const ALL_OPERATORS = [
  "is",
  "is_not",
  "is_any_of",
  "contains",
  "does_not_contain",
  "starts_with",
  "ends_with",
  "gt",
  "gte",
  "lt",
  "lte",
  "before",
  "after",
  "on_or_before",
  "on_or_after",
  "is_empty",
  "is_not_empty",
] as const satisfies readonly Operator[];

function isOperator(value: unknown): value is Operator {
  return (ALL_OPERATORS as readonly unknown[]).includes(value);
}

function filterNode(raw: unknown): FilterNode | undefined {
  if (!isRecord(raw)) {
    return undefined;
  }
  for (const key of ["and", "or"] as const) {
    const items = raw[key];
    if (Array.isArray(items)) {
      const kept = (items as unknown[])
        .map((item) => filterNode(item))
        .filter((item): item is FilterNode => item !== undefined);
      return key === "and" ? { and: kept } : { or: kept };
    }
  }
  const property = text(raw["property"]);
  const operator = raw["operator"];
  if (property === undefined || !isOperator(operator)) {
    return undefined;
  }
  return raw["value"] === undefined
    ? { property, operator }
    : { property, operator, value: raw["value"] };
}

function list<TItem>(raw: unknown, read: (item: unknown) => TItem | undefined): TItem[] {
  return Array.isArray(raw)
    ? (raw as unknown[])
        .map((item) => read(item))
        .filter((item): item is TItem => item !== undefined)
    : [];
}

/** A view's config, read defensively: what does not fit is left out. */
export function viewConfig(view: Pick<TableViewView, "config">): ViewConfig {
  const raw = isRecord(view.config) ? view.config : {};
  const groupBy = text(raw["group_by"]);
  return {
    filter: filterNode(raw["filter"]),
    sorts: list(raw["sorts"], (item): SortSpec | undefined => {
      if (!isRecord(item)) {
        return undefined;
      }
      const property = text(item["property"]);
      const direction = SORT_DIRECTIONS.find((candidate) => candidate === item["direction"]);
      return property === undefined || direction === undefined
        ? undefined
        : { property, direction };
    }),
    summaries: list(raw["summaries"], (item): SummarySpec | undefined => {
      if (!isRecord(item)) {
        return undefined;
      }
      const property = text(item["property"]);
      const kind = SUMMARY_KINDS.find((candidate) => candidate === item["kind"]);
      return property === undefined || kind === undefined ? undefined : { property, kind };
    }),
    properties: list(raw["properties"], (item): ColumnSpec | undefined => {
      if (!isRecord(item)) {
        return undefined;
      }
      const property = text(item["property"]);
      if (property === undefined) {
        return undefined;
      }
      const width = typeof item["width"] === "number" ? item["width"] : undefined;
      const visible = typeof item["visible"] === "boolean" ? item["visible"] : undefined;
      return { property, visible, width };
    }),
    ...(groupBy === undefined ? {} : { groupBy }),
    hiddenGroups: list(raw["hidden_groups"], text),
    cardProperties: list(raw["card_properties"], text),
  };
}

/** A config as the server takes it: empty parts left out, a board's groups kept. */
export function configJson(config: ViewConfig, board: boolean): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (config.filter !== undefined && !isEmptyFilter(config.filter)) {
    out["filter"] = config.filter;
  }
  if (config.sorts.length > 0) {
    out["sorts"] = config.sorts;
  }
  if (config.summaries.length > 0) {
    out["summaries"] = config.summaries;
  }
  if (config.properties.length > 0) {
    out["properties"] = config.properties.map(({ property, visible, width }) => ({
      property,
      ...(visible === undefined ? {} : { visible }),
      ...(width === undefined ? {} : { width }),
    }));
  }
  if (board && config.groupBy !== undefined) {
    out["group_by"] = config.groupBy;
    if (config.hiddenGroups.length > 0) {
      out["hidden_groups"] = config.hiddenGroups;
    }
    if (config.cardProperties.length > 0) {
      out["card_properties"] = config.cardProperties;
    }
  }
  return out;
}

// -------------------------------------------------------------------- filters

function isEmptyFilter(node: FilterNode): boolean {
  if ("and" in node) {
    return node.and.every((item) => isEmptyFilter(item));
  }
  if ("or" in node) {
    return node.or.every((item) => isEmptyFilter(item));
  }
  return false;
}

/**
 * A filter as the builder edits it: one group of conditions, all of them
 * (`and`) or any (`or`). A filter nested deeper than that is `nested`: the
 * builder shows it and can clear it, but not edit it.
 */
export interface FlatFilter {
  mode: "and" | "or";
  conditions: Condition[];
  nested: boolean;
}

export function flatFilter(filter: FilterNode | undefined): FlatFilter {
  if (filter === undefined) {
    return { mode: "and", conditions: [], nested: false };
  }
  if (!("and" in filter) && !("or" in filter)) {
    return { mode: "and", conditions: [filter], nested: false };
  }
  const mode = "and" in filter ? "and" : "or";
  const items = "and" in filter ? filter.and : filter.or;
  const conditions: Condition[] = [];
  let nested = false;
  for (const item of items) {
    if ("and" in item || "or" in item) {
      const inner = flatFilter(item);
      if (inner.conditions.length === 0) {
        continue;
      }
      // A one-condition group is that condition; any other is nested.
      if (inner.conditions.length === 1 && !inner.nested) {
        conditions.push(...inner.conditions);
      } else {
        nested = true;
      }
    } else {
      conditions.push(item);
    }
  }
  return { mode, conditions, nested };
}

/** The filter a flat one stands for; nothing when it has no conditions. */
export function filterOf(flat: Pick<FlatFilter, "mode" | "conditions">): FilterNode | undefined {
  if (flat.conditions.length === 0) {
    return undefined;
  }
  return flat.mode === "and" ? { and: flat.conditions } : { or: flat.conditions };
}

/** Whether an operator takes a value (`is empty` does not). */
export function takesValue(operator: Operator): boolean {
  return operator !== "is_empty" && operator !== "is_not_empty";
}

const TEXTUAL: readonly Operator[] = [
  "is",
  "is_not",
  "contains",
  "does_not_contain",
  "starts_with",
  "ends_with",
  "is_empty",
  "is_not_empty",
];
const NUMERIC: readonly Operator[] = [
  "is",
  "is_not",
  "gt",
  "gte",
  "lt",
  "lte",
  "is_empty",
  "is_not_empty",
];
const CHOICE: readonly Operator[] = ["is", "is_not", "is_any_of", "is_empty", "is_not_empty"];
const MEMBERS: readonly Operator[] = ["contains", "does_not_contain", "is_empty", "is_not_empty"];
const DATES: readonly Operator[] = [
  "is",
  "before",
  "after",
  "on_or_before",
  "on_or_after",
  "is_empty",
  "is_not_empty",
];

/** Whether a created/edited property shows who (not when). */
export function showsWho(property: Pick<PropertyView, "config">): boolean {
  return isRecord(property.config) && property.config["of"] === "who";
}

/** The operators the server accepts for a property (`properties::filter::operators`). */
export function operatorsFor(property: Pick<PropertyView, "type" | "config">): readonly Operator[] {
  switch (property.type) {
    case "title":
    case "text":
    case "url":
    case "email":
    case "phone": {
      return TEXTUAL;
    }
    case "number":
    case "unique_id": {
      return NUMERIC;
    }
    case "select":
    case "status": {
      return CHOICE;
    }
    case "multi_select":
    case "person":
    case "relation":
    case "campbooks_link": {
      return MEMBERS;
    }
    case "date": {
      return DATES;
    }
    case "created":
    case "edited": {
      return showsWho(property) ? ["is", "is_not"] : DATES.slice(0, 5);
    }
    case "checkbox": {
      return ["is"];
    }
    case "files": {
      return ["is_empty", "is_not_empty"];
    }
    default: {
      return [];
    }
  }
}

/** Whether a property can be filtered on at all. */
export function filterable(property: Pick<PropertyView, "type" | "config">): boolean {
  return operatorsFor(property).length > 0;
}

/** The value a new condition starts with, for a property and operator. */
export function startingValue(
  property: Pick<PropertyView, "type" | "config">,
  operator: Operator,
): unknown {
  if (!takesValue(operator)) {
    return undefined;
  }
  switch (property.type) {
    case "checkbox": {
      return true;
    }
    case "select":
    case "status":
    case "multi_select": {
      if (operator === "is_any_of") {
        return [];
      }
      return optionsOf(property)[0]?.id ?? "";
    }
    case "campbooks_link": {
      return undefined;
    }
    default: {
      return "";
    }
  }
}

/** Whether a condition is complete enough to send. */
export function conditionReady(condition: Condition): boolean {
  if (!takesValue(condition.operator)) {
    return true;
  }
  const value = condition.value;
  if (typeof value === "string") {
    return value.trim() !== "";
  }
  if (Array.isArray(value)) {
    return value.length > 0;
  }
  return value !== undefined && value !== null;
}

// ------------------------------------------------------------------ summaries

/** The summaries a type can show (`properties::filter::summary_allowed`). */
export function summaryKinds(type: PropertyType): readonly SummaryKind[] {
  const kinds: SummaryKind[] = ["count"];
  if (type === "number") {
    kinds.push("sum", "average");
  }
  if (["number", "date", "created", "edited", "unique_id"].includes(type)) {
    kinds.push("min", "max");
  }
  if (type === "checkbox") {
    kinds.push("percent_checked");
  }
  return kinds;
}

// -------------------------------------------------------------------- types

/** Whether rows can be ordered by a property. */
export function sortable(property: Pick<PropertyView, "type" | "config">): boolean {
  switch (property.type) {
    case "title":
    case "text":
    case "number":
    case "select":
    case "status":
    case "date":
    case "checkbox":
    case "url":
    case "email":
    case "phone":
    case "unique_id": {
      return true;
    }
    case "created":
    case "edited": {
      return !showsWho(property);
    }
    default: {
      return false;
    }
  }
}

/** Whether a board can group by a type. */
export function groupable(type: PropertyType): boolean {
  return type === "select" || type === "status";
}

/** Whether a member can write the cells of a type by hand. */
export function editable(type: PropertyType): boolean {
  return !(
    type === "title" ||
    type === "created" ||
    type === "edited" ||
    type === "unique_id" ||
    isReserved(type)
  );
}

/** The types only a Notion sync makes: shown, never written. */
function isReserved(type: PropertyType): boolean {
  return (
    type === "notion_formula" ||
    type === "notion_rollup" ||
    type === "notion_people" ||
    type === "external"
  );
}

// -------------------------------------------------------------------- options

const STATUS_GROUPS = ["todo", "doing", "done"] as const;

export function colorOf(color: string | null | undefined): OptionColor {
  return OPTION_COLORS.find((candidate) => candidate === color) ?? "default";
}

/** A select, multi-select or status property's options, in order. */
export function optionsOf(property: Pick<PropertyView, "config">): ChoiceOption[] {
  const config = property.config;
  const raw = isRecord(config) ? config["options"] : undefined;
  return list(raw, (item): ChoiceOption | undefined => {
    if (!isRecord(item)) {
      return undefined;
    }
    const name = text(item["name"]);
    const id = text(item["id"]);
    if (name === undefined || id === undefined) {
      return undefined;
    }
    const group = STATUS_GROUPS.find((candidate) => candidate === item["group"]);
    return {
      id,
      name,
      color: colorOf(text(item["color"])),
      ...(group === undefined ? {} : { group }),
    };
  });
}

export function optionById(property: Pick<PropertyView, "config">, id: string | undefined) {
  return id === undefined ? undefined : optionsOf(property).find((option) => option.id === id);
}

/** The colour a new option takes: the next one round the palette. */
export function nextColor(options: readonly ChoiceOption[]): OptionColor {
  const palette = OPTION_COLORS.filter((color) => color !== "default");
  return palette[options.length % palette.length] ?? "gray";
}

// --------------------------------------------------------------------- values

/** A text cell's runs as plain text. */
export function plainText(value: unknown): string {
  if (!Array.isArray(value)) {
    return "";
  }
  return (value as unknown[])
    .map((run) => (isRecord(run) ? (text(run["text"]) ?? "") : ""))
    .join("");
}

/** A plain string as a text cell's value; nothing when blank. */
export function runsOf(content: string): unknown {
  const trimmed = content.trim();
  return trimmed === "" ? null : [{ text: trimmed }];
}

/** The ids a person, relation, select or multi-select cell holds. */
export function idsOf(value: unknown): string[] {
  if (typeof value === "string") {
    return [value];
  }
  return list(value, text);
}

export interface LinkRef {
  type: string;
  id: string;
}

/** The records a Campbooks link cell names. */
export function linksOf(value: unknown): LinkRef[] {
  return list(value, (item): LinkRef | undefined => {
    if (!isRecord(item)) {
      return undefined;
    }
    const type = text(item["type"]);
    const id = text(item["id"]);
    return type === undefined || id === undefined ? undefined : { type, id };
  });
}

export interface FileRef {
  hash: string;
  name: string;
  size: number;
  type: string;
}

export function filesOf(value: unknown): FileRef[] {
  return list(value, (item): FileRef | undefined => {
    if (!isRecord(item)) {
      return undefined;
    }
    const hash = text(item["hash"]);
    const name = text(item["name"]);
    const type = text(item["type"]);
    const size = item["size"];
    return hash === undefined ||
      name === undefined ||
      type === undefined ||
      typeof size !== "number"
      ? undefined
      : { hash, name, size, type };
  });
}

/** A date cell: a day or an instant, and optionally where it ends. */
export interface DateValue {
  start: string;
  end?: string | undefined;
  timeZone?: string | undefined;
}

export function dateOf(value: unknown): DateValue | undefined {
  if (!isRecord(value)) {
    return undefined;
  }
  const start = text(value["start"]);
  if (start === undefined) {
    return undefined;
  }
  return { start, end: text(value["end"]), timeZone: text(value["time_zone"]) };
}

/** The wire value of a date. */
export function dateJson(date: DateValue): Record<string, string> {
  return {
    start: date.start,
    ...(date.end === undefined ? {} : { end: date.end }),
    ...(date.timeZone === undefined ? {} : { time_zone: date.timeZone }),
  };
}

/** Whether a date string carries a time (not just a day). */
function hasTime(value: string): boolean {
  return value.includes("T");
}

const two = (n: number) => String(n).padStart(2, "0");

/** The day (`YYYY-MM-DD`) of a date or instant, as the viewer's clock reads it. */
export function dayOf(value: string): string {
  if (!hasTime(value)) {
    return value;
  }
  const at = new Date(value);
  return Number.isNaN(at.getTime())
    ? value.slice(0, 10)
    : `${String(at.getFullYear())}-${two(at.getMonth() + 1)}-${two(at.getDate())}`;
}

/** The `HH:MM` of an instant, as the viewer's clock reads it; nothing for a day. */
export function timeOf(value: string): string | undefined {
  if (!hasTime(value)) {
    return undefined;
  }
  const at = new Date(value);
  return Number.isNaN(at.getTime()) ? undefined : `${two(at.getHours())}:${two(at.getMinutes())}`;
}

/** A day and a time of day (the viewer's) as the value the server keeps. */
export function joinWhen(day: string, time: string | undefined): string {
  if (time === undefined) {
    return day;
  }
  const at = new Date(`${day}T${time}:00`);
  return Number.isNaN(at.getTime()) ? day : at.toISOString().replace(/\.\d{3}Z$/u, "Z");
}

/** An empty cell value: nothing, an empty list, blank text, false is NOT empty. */
export function isEmptyValue(value: unknown): boolean {
  return (
    value === null ||
    value === undefined ||
    value === "" ||
    (Array.isArray(value) && (value as unknown[]).length === 0)
  );
}

const NUMBER_FORMAT = /^-?\d{1,15}(\.\d{1,9})?$/u;

/** A typed number as the server's decimal string, or nothing when it is not one. */
export function decimalOf(input: string): string | undefined {
  const normal = input.trim().replace(",", ".");
  return NUMBER_FORMAT.test(normal) ? normal : undefined;
}

/** A number cell shown in its format (plain, percent, or a currency) in the viewer's language. */
export function formatNumber(
  value: unknown,
  property: Pick<PropertyView, "config">,
  locale: string,
): string {
  const raw = typeof value === "number" ? String(value) : text(value);
  if (raw === undefined || !NUMBER_FORMAT.test(raw)) {
    return "";
  }
  const amount = Number(raw);
  const config = isRecord(property.config) ? property.config : {};
  const format = text(config["format"]);
  const currency = text(config["currency"]);
  try {
    if (format === "currency" && currency !== undefined) {
      return new Intl.NumberFormat(locale, { style: "currency", currency }).format(amount);
    }
    if (format === "percent") {
      return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 9 }).format(amount)}\u202F%`;
    }
    return new Intl.NumberFormat(locale, { maximumFractionDigits: 9 }).format(amount);
  } catch {
    return raw;
  }
}

/** A day or instant in the viewer's language: "Thu, 30 Oct", with the time when it has one. */
function formatWhen(value: string, locale: string): string {
  const withTime = hasTime(value);
  const date = new Date(withTime ? value : `${value}T12:00:00`);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
    ...(withTime ? { hour: "numeric", minute: "2-digit" } : {}),
  }).format(date);
}

/** A date cell, "30 Oct → 3 Nov" for a range. */
export function formatDate(value: unknown, locale: string): string {
  const date = dateOf(value);
  if (date === undefined) {
    return "";
  }
  const start = formatWhen(date.start, locale);
  return date.end === undefined ? start : `${start} → ${formatWhen(date.end, locale)}`;
}

/** A moment (an ISO instant) as a date and time. */
export function formatInstant(value: string, locale: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

// ---------------------------------------------------------------------- links

const ALLOWED_SCHEMES = new Set(["https:", "http:", "mailto:"]);

/** An address a cell may link to: https, http and mailto only. Anything else is text. */
export function safeHref(address: string): string | undefined {
  const trimmed = address.trim();
  if (trimmed === "" || /\s/u.test(trimmed)) {
    return undefined;
  }
  try {
    const url = new URL(trimmed);
    return ALLOWED_SCHEMES.has(url.protocol) ? url.href : undefined;
  } catch {
    return undefined;
  }
}

/** The address a url, email or text-run link cell opens: a mail address becomes mailto:. */
export function hrefOf(type: PropertyType, value: unknown): string | undefined {
  const content = text(value);
  if (content === undefined) {
    return undefined;
  }
  if (type === "email") {
    return safeHref(`mailto:${content}`);
  }
  return type === "url" ? safeHref(content) : undefined;
}

// -------------------------------------------------------------------- columns

/** A property's column in a view: where it sits, how wide, whether it shows. */
export interface Column {
  property: PropertyView;
  width: number;
  visible: boolean;
}

const WIDTHS: Partial<Record<PropertyType, number>> = {
  title: 240,
  checkbox: 96,
  number: 128,
  date: 168,
  select: 160,
  status: 160,
  multi_select: 220,
  person: 176,
  files: 200,
  relation: 220,
  campbooks_link: 240,
  text: 220,
  url: 200,
  email: 200,
  phone: 160,
  unique_id: 96,
  created: 176,
  edited: 176,
};

const MIN_WIDTH = 64;
const MAX_WIDTH = 640;

function defaultWidth(type: PropertyType): number {
  return WIDTHS[type] ?? 180;
}

/** The columns of a view: the view's order and widths first, then any property it does not name. */
export function columnsOf(properties: readonly PropertyView[], config: ViewConfig): Column[] {
  const byId = new Map(properties.map((property) => [property.id, property]));
  const seen = new Set<string>();
  const columns: Column[] = [];
  for (const spec of config.properties) {
    const property = byId.get(spec.property);
    if (property === undefined || seen.has(property.id)) {
      continue;
    }
    seen.add(property.id);
    columns.push({
      property,
      width: clampWidth(spec.width ?? defaultWidth(property.type)),
      // The title is the row's name: it always shows.
      visible: property.type === "title" || spec.visible !== false,
    });
  }
  for (const property of properties.toSorted((a, b) => a.position.localeCompare(b.position))) {
    if (!seen.has(property.id)) {
      columns.push({ property, width: defaultWidth(property.type), visible: true });
    }
  }
  // The title leads.
  const title = columns.findIndex((column) => column.property.type === "title");
  if (title > 0) {
    const [first] = columns.splice(title, 1);
    if (first !== undefined) {
      columns.unshift(first);
    }
  }
  return columns;
}

function clampWidth(width: number): number {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(width)));
}

/** The view's `properties` list for these columns: every column named, in order. */
export function specsOf(columns: readonly Column[]): ColumnSpec[] {
  return columns.map((column) => ({
    property: column.property.id,
    visible: column.visible,
    width: column.width,
  }));
}

/** A property's name for a summary row: "Sum", "Count"... is the caller's i18n; this is the key. */
export function summaryKey(
  kind: SummaryKind,
): "count" | "sum" | "average" | "min" | "max" | "percentChecked" {
  return kind === "percent_checked" ? "percentChecked" : kind;
}

/** A summary's figure shown in the property's format. */
export function formatSummary(
  kind: SummaryKind,
  value: string | null | undefined,
  property: Pick<PropertyView, "type" | "config">,
  locale: string,
): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (kind === "count") {
    return Number(value).toLocaleString(locale);
  }
  if (kind === "percent_checked") {
    return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(Number(value))}\u202F%`;
  }
  if (property.type === "date" || property.type === "created" || property.type === "edited") {
    return hasTime(value) ? formatInstant(value, locale) : formatWhen(value, locale);
  }
  return formatNumber(value, property, locale);
}
