import type { Schema } from "~/lib/api";

export type TableDetail = Schema<"TableDetail">;
export type PropertyView = Schema<"PropertyView">;
export type PropertyType = Schema<"PropertyType">;
export type TableViewView = Schema<"TableViewView">;
export type TableViewType = Schema<"TableViewType">;
export type RowView = Schema<"RowView">;
export type TableRows = Schema<"TableRows">;
export type TableQueryBody = Schema<"TableQuery">;
export type CellInput = Schema<"CellInput">;
export type CellsWritten = Schema<"CellsWritten">;
export type SummaryValue = Schema<"SummaryValue">;
export type SummaryKind = Schema<"SummaryKind">;
export type GroupSize = Schema<"GroupSize">;
type StatusGroup = Schema<"StatusGroup">;
export type SortDirection = "asc" | "desc";

/** How a condition compares a cell with its operand (the server's operators). */
export type Operator =
  | "is"
  | "is_not"
  | "is_any_of"
  | "contains"
  | "does_not_contain"
  | "starts_with"
  | "ends_with"
  | "gt"
  | "gte"
  | "lt"
  | "lte"
  | "before"
  | "after"
  | "on_or_before"
  | "on_or_after"
  | "is_empty"
  | "is_not_empty";

/** An option of a select, as the screens read it: always with its id. */
/** An option to write: one without an id is new. */
export type OptionDraft = Omit<ChoiceOption, "id"> & { id?: string };

export interface ChoiceOption {
  id: string;
  name: string;
  color: OptionColor;
  group?: StatusGroup;
}
export type NewProperty = Schema<"NewProperty">;
export type NewView = Schema<"NewView">;
export type AddedProperty = Schema<"AddedProperty">;

/** The colours of an option, as the server names them. */
export const OPTION_COLORS = [
  "default",
  "gray",
  "brown",
  "orange",
  "yellow",
  "green",
  "blue",
  "purple",
  "pink",
  "red",
] as const;

export type OptionColor = (typeof OPTION_COLORS)[number];

/** A condition of a filter: one property compared with one operand. */
export interface Condition {
  property: string;
  operator: Operator;
  value?: unknown;
}

/** A filter: AND/OR groups of conditions. */
export type FilterNode = Condition | { and: FilterNode[] } | { or: FilterNode[] };

export interface SortSpec {
  property: string;
  direction: SortDirection;
}

export interface SummarySpec {
  property: string;
  kind: SummaryKind;
}

/** A column of a view: where it sits, whether it shows, how wide. */
export interface ColumnSpec {
  property: string;
  visible?: boolean | undefined;
  width?: number | undefined;
}

/** A view's config as the screens read it (the wire holds it as free JSON). */
export interface ViewConfig {
  filter?: FilterNode | undefined;
  sorts: SortSpec[];
  summaries: SummarySpec[];
  properties: ColumnSpec[];
  groupBy?: string | undefined;
  hiddenGroups: string[];
  cardProperties: string[];
}

/** What the type menu offers, in three groups. */
export const TYPE_GROUPS = {
  basic: [
    "text",
    "number",
    "select",
    "multi_select",
    "status",
    "date",
    "checkbox",
    "person",
    "files",
    "url",
    "email",
    "phone",
  ],
  links: ["relation", "campbooks_link"],
  filled: ["created", "edited", "unique_id"],
} as const satisfies Record<string, readonly PropertyType[]>;

export type TypeGroup = keyof typeof TYPE_GROUPS;
