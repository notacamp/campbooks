// Fictional tables for tests and stories (never a real person): Duarte &
// Pessoa's clients, and a small server for them that keeps state (cells have
// versions, properties and views change, rows come and go) the way the
// pages API's table operations answer.
import * as Y from "yjs";

import type { Schema } from "~/lib/api";

import { toBase64 } from "../../src/modules/pages/editor/bytes";

import { MARTA } from "./attention";
import { LEASE } from "./tasks";
import { ANA, RUI, SOFIA } from "./workspaces";

type Summary = Schema<"PageSummary">;

const id = (n: number) => `0192f1a0-0000-7000-8000-00000000${n.toString(16).padStart(4, "0")}`;

export const CLIENTS_ID = id(28_673);
export const PROJECTS_ID = id(28_674);

/** The property ids of the clients table. */
export const P = {
  name: id(28_929),
  stage: id(28_930),
  owner: id(28_931),
  next: id(28_932),
  value: id(28_933),
  threads: id(28_934),
  signed: id(28_935),
  projects: id(28_936),
} as const;

export const OPTIONS = {
  lead: "o-lead",
  proposal: "o-proposal",
  kickoff: "o-kickoff",
  active: "o-active",
  paused: "o-paused",
} as const;

export const VIEW_TABLE = id(29_185);
export const VIEW_BOARD = id(29_186);

/** The rows of the clients table. */
export const ROW = {
  acme: id(29_441),
  lumen: id(29_442),
  northwind: id(29_443),
  harbour: id(29_444),
  oldmill: id(29_445),
} as const;

export const PROJECT_ROW = id(29_697);
const PROJECTS_VIEW = id(29_698);
const PROJECTS_TITLE = id(29_699);

const summary = (page: Partial<Summary> & { id: string; title: string }): Summary => ({
  parent: null,
  place: "workspace",
  position: "a0",
  icon: null,
  has_children: false,
  role: "manager",
  created_by: ANA.user_id,
  edited_at: "2026-10-20T09:00:00Z",
  trashed_at: null,
  meta_revision: 1,
  kind: "table",
  ...page,
});

const CLIENTS_PAGE = summary({ id: CLIENTS_ID, title: "Clients", position: "a5" });
const PROJECTS_PAGE = summary({ id: PROJECTS_ID, title: "Projects", position: "a6" });

interface Cell {
  value: unknown;
  version: number;
}
interface Row {
  id: string;
  table: string;
  number: number;
  title: string;
  cells: Record<string, Cell>;
  created_by: string;
  created_at: string;
  edited_by: string;
  edited_at: string;
  revision: number;
  meta_revision: number;
  trashed: boolean;
}
interface Property {
  id: string;
  name: string;
  type: string;
  config: Record<string, unknown>;
  position: string;
  can_revert: boolean;
  removed?: boolean;
}
interface View {
  id: string;
  name: string;
  type: string;
  position: string;
  config: Record<string, unknown>;
}
interface Table {
  page: Summary;
  properties: Property[];
  views: View[];
  rows: Row[];
  revision: number;
}

const option = (key: keyof typeof OPTIONS, name: string, color: string) => ({
  id: OPTIONS[key],
  name,
  color,
});

const cells = (values: Record<string, unknown>): Record<string, Cell> =>
  Object.fromEntries(Object.entries(values).map(([key, value]) => [key, { value, version: 1 }]));

const row = (
  rowId: string,
  table: string,
  number: number,
  title: string,
  values: Record<string, unknown>,
): Row => ({
  id: rowId,
  table,
  number,
  title,
  cells: cells(values),
  created_by: ANA.user_id,
  created_at: "2026-10-01T09:00:00Z",
  edited_by: ANA.user_id,
  edited_at: "2026-10-20T09:00:00Z",
  revision: 1,
  meta_revision: 1,
  trashed: false,
});

function fresh(): Record<string, Table> {
  const clients: Table = {
    page: CLIENTS_PAGE,
    revision: 1,
    properties: [
      { id: P.name, name: "Name", type: "title", config: {}, position: "a0", can_revert: false },
      {
        id: P.stage,
        name: "Stage",
        type: "select",
        position: "a1",
        can_revert: false,
        config: {
          options: [
            option("lead", "Lead", "gray"),
            option("proposal", "Proposal sent", "yellow"),
            option("kickoff", "Kickoff", "blue"),
            option("active", "Active", "green"),
            option("paused", "Paused", "red"),
          ],
        },
      },
      { id: P.owner, name: "Owner", type: "person", config: {}, position: "a2", can_revert: false },
      {
        id: P.next,
        name: "Next date",
        type: "date",
        config: { time: false },
        position: "a3",
        can_revert: false,
      },
      {
        id: P.value,
        name: "Value",
        type: "number",
        config: { format: "currency", currency: "EUR" },
        position: "a4",
        can_revert: false,
      },
      {
        id: P.threads,
        name: "Threads",
        type: "campbooks_link",
        config: { types: [] },
        position: "a5",
        can_revert: false,
      },
      {
        id: P.signed,
        name: "Signed",
        type: "checkbox",
        config: {},
        position: "a6",
        can_revert: false,
      },
      {
        id: P.projects,
        name: "Projects",
        type: "relation",
        config: { table: PROJECTS_ID, pair: null },
        position: "a7",
        can_revert: false,
      },
    ],
    views: [
      {
        id: VIEW_TABLE,
        name: "All clients",
        type: "table",
        position: "a0",
        config: {
          filter: { and: [{ property: P.stage, operator: "is_not", value: OPTIONS.paused }] },
          summaries: [
            { property: P.value, kind: "sum" },
            { property: P.signed, kind: "percent_checked" },
          ],
        },
      },
      {
        id: VIEW_BOARD,
        name: "By stage",
        type: "board",
        position: "a1",
        config: {
          group_by: P.stage,
          summaries: [{ property: P.value, kind: "sum" }],
          filter: { and: [{ property: P.stage, operator: "is_not", value: OPTIONS.paused }] },
        },
      },
    ],
    rows: [
      row(ROW.acme, CLIENTS_ID, 1, "Acme Studio", {
        [P.stage]: OPTIONS.kickoff,
        [P.owner]: [ANA.user_id],
        [P.next]: { start: "2026-10-30" },
        [P.value]: "9600",
        [P.threads]: [
          { type: "conversations.conversation", id: MARTA.id },
          { type: "tasks.task", id: LEASE.id },
        ],
        [P.signed]: true,
        [P.projects]: [PROJECT_ROW],
      }),
      row(ROW.lumen, CLIENTS_ID, 2, "Lumen Dental", {
        [P.stage]: OPTIONS.active,
        [P.owner]: [RUI.user_id],
        [P.next]: { start: "2026-11-03" },
        [P.value]: "12000",
        [P.signed]: true,
      }),
      row(ROW.northwind, CLIENTS_ID, 3, "Northwind Bakery", {
        [P.stage]: OPTIONS.proposal,
        [P.owner]: [ANA.user_id],
        [P.next]: { start: "2026-10-31" },
        [P.value]: "4800",
        [P.signed]: false,
      }),
      row(ROW.harbour, CLIENTS_ID, 4, "Harbour Physio", {
        [P.stage]: OPTIONS.lead,
        [P.owner]: [RUI.user_id],
        [P.next]: { start: "2026-11-05" },
        [P.value]: "2000",
        [P.signed]: false,
      }),
      row(ROW.oldmill, CLIENTS_ID, 5, "Old Mill Dental", {
        [P.stage]: OPTIONS.paused,
        [P.owner]: [SOFIA.user_id],
        [P.value]: "1500",
      }),
    ],
  };
  const projects: Table = {
    page: PROJECTS_PAGE,
    revision: 1,
    properties: [
      {
        id: PROJECTS_TITLE,
        name: "Name",
        type: "title",
        config: {},
        position: "a0",
        can_revert: false,
      },
    ],
    views: [{ id: PROJECTS_VIEW, name: "All projects", type: "table", position: "a0", config: {} }],
    rows: [row(PROJECT_ROW, PROJECTS_ID, 1, "Acme rebrand", {})],
  };
  return { [CLIENTS_ID]: clients, [PROJECTS_ID]: projects };
}

/** An object without some keys. */
function without(value: object, ...keys: string[]): Record<string, unknown> {
  return Object.fromEntries(Object.entries(value).filter(([key]) => !keys.includes(key)));
}

/** A row as the API shows it: without what the server keeps for itself. */
export function withoutTrashed(r: Row): Omit<Row, "trashed"> {
  return without(r, "trashed") as Omit<Row, "trashed">;
}

/** What a mocked endpoint answers (the shape both the tests' and the stories' mocks take). */
export interface Answer {
  status?: number;
  body?: unknown;
}
export type Handler = (body: unknown, url: URL) => Answer;

const text = (value: unknown): string => (typeof value === "string" ? value : "");
const record = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};
const list = (value: unknown): unknown[] => (Array.isArray(value) ? (value as unknown[]) : []);

/** The page-and-table API over state the calls change. */
export class TablesServer {
  tables = fresh();
  /** Row pages' bodies. */
  docs = new Map<string, Y.Doc>();
  /** Every call made, as "METHOD /path". */
  calls: string[] = [];
  /** Cells that someone else changes right before the next write to them (a race). */
  raceOn = new Set<string>();
  /** The next counter of made ids. */
  private next = 29_952;

  reset() {
    this.tables = fresh();
    this.docs.clear();
    this.calls = [];
    this.raceOn.clear();
  }

  private made() {
    this.next += 1;
    return id(this.next);
  }

  table(tableId: string): Table | undefined {
    return this.tables[tableId];
  }

  rowOf(rowId: string): Row | undefined {
    for (const table of Object.values(this.tables)) {
      const found = table.rows.find((candidate) => candidate.id === rowId);
      if (found !== undefined) {
        return found;
      }
    }
    return undefined;
  }

  private detail(table: Table) {
    return {
      page: table.page,
      properties: table.properties
        .filter((property) => property.removed !== true)
        .map((property) => without(property, "removed")),
      revision: table.revision,
      views: table.views,
      rows: table.rows.filter((candidate) => !candidate.trashed).length,
    };
  }

  private view(r: Row) {
    return withoutTrashed(r);
  }

  // ------------------------------------------------------------- the query

  private matches(table: Table, r: Row, node: unknown): boolean {
    const n = record(node);
    if (Array.isArray(n["and"])) {
      return list(n["and"]).every((item) => this.matches(table, r, item));
    }
    if (Array.isArray(n["or"])) {
      return list(n["or"]).some((item) => this.matches(table, r, item));
    }
    const property = table.properties.find((candidate) => candidate.id === n["property"]);
    if (property === undefined) {
      return true;
    }
    const raw = property.type === "title" ? r.title : r.cells[property.id]?.value;
    const operand = n["value"];
    const asText = (typeof raw === "string" ? raw : "").toLowerCase();
    const ids = Array.isArray(raw) ? raw.map((item) => (typeof item === "string" ? item : "")) : [];
    switch (n["operator"]) {
      case "is": {
        return property.type === "checkbox"
          ? raw === operand
          : asText === text(operand).toLowerCase();
      }
      case "is_not": {
        return raw !== operand;
      }
      case "is_any_of": {
        return list(operand).includes(raw);
      }
      case "contains": {
        return Array.isArray(raw)
          ? ids.includes(text(operand))
          : asText.includes(text(operand).toLowerCase());
      }
      case "does_not_contain": {
        return !asText.includes(text(operand).toLowerCase());
      }
      case "starts_with": {
        return asText.startsWith(text(operand).toLowerCase());
      }
      case "ends_with": {
        return asText.endsWith(text(operand).toLowerCase());
      }
      case "gt": {
        return Number(raw) > Number(operand);
      }
      case "gte": {
        return Number(raw) >= Number(operand);
      }
      case "lt": {
        return Number(raw) < Number(operand);
      }
      case "lte": {
        return Number(raw) <= Number(operand);
      }
      case "is_empty": {
        return (
          raw === undefined ||
          raw === null ||
          raw === "" ||
          (ids.length === 0 && Array.isArray(raw))
        );
      }
      case "is_not_empty": {
        return !(
          raw === undefined ||
          raw === null ||
          raw === "" ||
          (Array.isArray(raw) && raw.length === 0)
        );
      }
      default: {
        return true;
      }
    }
  }

  private sortValue(table: Table, r: Row, propertyId: string): string | number {
    const property = table.properties.find((candidate) => candidate.id === propertyId);
    if (property === undefined) {
      return "";
    }
    if (property.type === "title") {
      return r.title.toLowerCase();
    }
    const value = r.cells[propertyId]?.value;
    if (property.type === "number") {
      return Number(value ?? Number.NEGATIVE_INFINITY);
    }
    if (property.type === "date") {
      return text(record(value)["start"]);
    }
    if (property.type === "checkbox") {
      return value === true ? 1 : 0;
    }
    return typeof value === "string" ? value.toLowerCase() : "";
  }

  query(tableId: string, body: Record<string, unknown>): Answer {
    const table = this.tables[tableId];
    if (table === undefined) {
      return { status: 404 };
    }
    const view = table.views.find((candidate) => candidate.id === body["view"]);
    if (body["view"] !== undefined && body["view"] !== null && view === undefined) {
      return { status: 404 };
    }
    const config = record(view?.config);
    let rows = table.rows.filter((candidate) => !candidate.trashed);
    const only = body["rows"];
    if (Array.isArray(only)) {
      rows = rows.filter((candidate) => only.includes(candidate.id));
    }
    for (const filter of [config["filter"], body["filter"]]) {
      if (filter !== undefined && filter !== null) {
        rows = rows.filter((candidate) => this.matches(table, candidate, filter));
      }
    }
    const sorts = list(body["sorts"] ?? config["sorts"]).map(record);
    rows = rows.toSorted((a, b) => {
      for (const sort of sorts) {
        const left = this.sortValue(table, a, text(sort["property"]));
        const right = this.sortValue(table, b, text(sort["property"]));
        if (left !== right) {
          const order = left < right ? -1 : 1;
          return sort["direction"] === "desc" ? -order : order;
        }
      }
      return a.number - b.number;
    });
    const matching = rows.length;
    const offset = Number(text(body["cursor"]).slice(1)) || 0;
    const limit = Math.min(100, Number(body["limit"] ?? 100));
    const page = rows.slice(offset, offset + limit);
    const seen = offset + page.length;
    const summaries = list(body["summaries"] ?? config["summaries"])
      .map(record)
      .map((summary) => {
        const values = rows.map((candidate) => candidate.cells[text(summary["property"])]?.value);
        const numbers = values.map(Number).filter((n) => Number.isFinite(n) && n !== 0);
        let value: string | null = null;
        switch (summary["kind"]) {
          case "count": {
            value = String(rows.length);
            break;
          }
          case "sum": {
            value = String(numbers.reduce((total, n) => total + n, 0));
            break;
          }
          case "average": {
            value =
              numbers.length === 0
                ? null
                : String(numbers.reduce((t, n) => t + n, 0) / numbers.length);
            break;
          }
          case "min": {
            value = numbers.length === 0 ? null : String(Math.min(...numbers));
            break;
          }
          case "max": {
            value = numbers.length === 0 ? null : String(Math.max(...numbers));
            break;
          }
          case "percent_checked": {
            value =
              rows.length === 0
                ? null
                : String((values.filter((v) => v === true).length / rows.length) * 100);
            break;
          }
          default:
        }
        return { property: summary["property"], kind: summary["kind"], value };
      });
    const groupBy = view?.type === "board" ? text(config["group_by"]) : "";
    const groups =
      groupBy === ""
        ? null
        : [...new Set(rows.map((candidate) => text(candidate.cells[groupBy]?.value) || null))].map(
            (optionId) => ({
              option: optionId,
              rows: rows.filter(
                (candidate) => (text(candidate.cells[groupBy]?.value) || null) === optionId,
              ).length,
            }),
          );
    return {
      body: {
        rows: page.map((candidate) => this.view(candidate)),
        next_cursor: seen < matching ? `o${String(seen)}` : null,
        matching,
        summaries,
        groups,
      },
    };
  }

  // -------------------------------------------------------------- writes

  setCells(tableId: string, rowId: string, body: Record<string, unknown>): Answer {
    const r = this.table(tableId)?.rows.find((candidate) => candidate.id === rowId);
    if (r === undefined) {
      return { status: 404 };
    }
    const results = list(body["cells"])
      .map(record)
      .map((input) => {
        const property = text(input["property"]);
        // Somebody else's write lands first (the next write to this cell is stale).
        if (this.raceOn.delete(`${rowId}:${property}`)) {
          r.cells[property] = {
            value: r.cells[property]?.value ?? null,
            version: (r.cells[property]?.version ?? 0) + 1,
          };
        }
        const before = r.cells[property];
        const stale =
          input["version"] !== undefined &&
          input["version"] !== null &&
          before !== undefined &&
          input["version"] !== before.version;
        const version = (before?.version ?? 0) + 1;
        if (input["value"] === null || input["value"] === undefined) {
          r.cells = Object.fromEntries(Object.entries(r.cells).filter(([key]) => key !== property));
        } else {
          r.cells[property] = { value: input["value"], version };
        }
        return {
          property,
          version,
          stale,
          previous: before?.value ?? null,
          previous_version: before?.version ?? 0,
        };
      });
    r.revision += 1;
    return { body: { results, row: this.view(r) } };
  }

  addRow(tableId: string, body: Record<string, unknown>): Answer {
    const table = this.table(tableId);
    if (table === undefined) {
      return { status: 404 };
    }
    const r = row(this.made(), tableId, table.rows.length + 1, text(body["title"]), {});
    for (const input of list(body["cells"]).map(record)) {
      r.cells[text(input["property"])] = { value: input["value"], version: 1 };
    }
    table.rows.push(r);
    return { status: 201, body: this.view(r) };
  }

  addProperty(tableId: string, body: Record<string, unknown>): Answer {
    const table = this.table(tableId);
    if (table === undefined) {
      return { status: 404 };
    }
    const property: Property = {
      id: this.made(),
      name: text(body["name"]),
      type: text(body["type"]),
      config: record(body["config"]),
      position: `a${String(table.properties.length)}`,
      can_revert: false,
    };
    if (property.type === "select" || property.type === "multi_select") {
      property.config = { options: [], ...property.config };
    }
    table.properties.push(property);
    table.revision += 1;
    const rest = without(property, "removed");
    return { status: 201, body: { property: rest } };
  }

  private property(tableId: string, propertyId: string): Property | undefined {
    return this.table(tableId)?.properties.find((candidate) => candidate.id === propertyId);
  }

  updateProperty(tableId: string, propertyId: string, body: Record<string, unknown>): Answer {
    const property = this.property(tableId, propertyId);
    if (property === undefined) {
      return { status: 404 };
    }
    if (typeof body["name"] === "string") {
      property.name = body["name"];
    }
    if (typeof body["type"] === "string" && body["type"] !== property.type) {
      property.type = body["type"];
      property.can_revert = true;
      if (body["config"] === undefined) {
        property.config = {};
      }
    }
    if (body["config"] !== undefined) {
      property.config = record(body["config"]);
    }
    const rest = without(property, "removed");
    return { body: rest };
  }

  /** Cells keep their values when a type changes back (the server keeps them for 30 days). */
  setOptions(tableId: string, propertyId: string, body: Record<string, unknown>): Answer {
    const property = this.property(tableId, propertyId);
    if (property === undefined) {
      return { status: 404 };
    }
    property.config = {
      ...property.config,
      options: list(body["options"])
        .map(record)
        .map((item) => ({
          id: typeof item["id"] === "string" ? item["id"] : `o-${this.made().slice(-4)}`,
          name: item["name"],
          color: item["color"] ?? "default",
          ...(item["group"] === undefined || item["group"] === null
            ? {}
            : { group: item["group"] }),
        })),
    };
    const rest = without(property, "removed");
    return { body: rest };
  }

  removeProperty(tableId: string, propertyId: string, removed: boolean): Answer {
    const property = this.property(tableId, propertyId);
    if (property === undefined) {
      return { status: 404 };
    }
    property.removed = removed;
    const rest = without(property, "removed");
    return { body: rest };
  }

  createView(tableId: string, body: Record<string, unknown>): Answer {
    const table = this.table(tableId);
    if (table === undefined) {
      return { status: 404 };
    }
    const view: View = {
      id: typeof body["id"] === "string" ? body["id"] : this.made(),
      name: text(body["name"]),
      type: text(body["type"]),
      position:
        typeof body["position"] === "string" ? body["position"] : `a${String(table.views.length)}`,
      config: record(body["config"]),
    };
    table.views.push(view);
    return { status: 201, body: view };
  }

  updateView(tableId: string, viewId: string, body: Record<string, unknown>): Answer {
    const view = this.table(tableId)?.views.find((candidate) => candidate.id === viewId);
    if (view === undefined) {
      return { status: 404 };
    }
    if (typeof body["name"] === "string") {
      view.name = body["name"];
    }
    if (body["config"] !== undefined) {
      view.config = record(body["config"]);
    }
    return { body: view };
  }

  removeView(tableId: string, viewId: string): Answer {
    const table = this.table(tableId);
    const view = table?.views.find((candidate) => candidate.id === viewId);
    if (table === undefined || view === undefined) {
      return { status: 404 };
    }
    table.views = table.views.filter((candidate) => candidate.id !== viewId);
    return { body: view };
  }

  // ------------------------------------------------------------- the pages

  /** Every page the tree lists: the tables, as `pages.list` shows them. */
  pages(extra: readonly Summary[] = []): Summary[] {
    return [...extra, ...Object.values(this.tables).map((table) => table.page)];
  }

  private rowPage(r: Row) {
    return {
      id: r.id,
      parent: r.table,
      place: "workspace",
      position: "a0",
      title: r.title,
      icon: null,
      role: "manager",
      created_by: r.created_by,
      created_at: r.created_at,
      edited_by: r.edited_by,
      edited_at: r.edited_at,
      trashed_at: r.trashed ? "2026-10-21T09:00:00Z" : null,
      revision: r.revision,
      meta_revision: r.meta_revision,
      kind: "row",
      grants: [],
      ancestors: [{ id: r.table, title: this.table(r.table)?.page.title ?? "", icon: null }],
      document: this.document(r.id),
    };
  }

  document(pageId: string) {
    let doc = this.docs.get(pageId);
    if (doc === undefined) {
      doc = new Y.Doc();
      this.docs.set(pageId, doc);
    }
    return {
      update: toBase64(Y.encodeStateAsUpdate(doc)),
      state_vector: toBase64(Y.encodeStateVector(doc)),
    };
  }

  tablePage(tableId: string) {
    const table = this.table(tableId);
    return table === undefined
      ? undefined
      : {
          ...table.page,
          created_at: "2026-10-01T09:00:00Z",
          edited_by: ANA.user_id,
          revision: table.revision,
          ancestors: [],
          grants: [],
          document: this.document(tableId),
        };
  }

  getPage(pageId: string): Answer {
    const table = this.tablePage(pageId);
    if (table !== undefined) {
      return { body: table };
    }
    const r = this.rowOf(pageId);
    return r === undefined ? { status: 404 } : { body: this.rowPage(r) };
  }

  rename(pageId: string, body: Record<string, unknown>): Answer {
    const r = this.rowOf(pageId);
    if (r !== undefined) {
      r.title = text(body["title"]);
      r.meta_revision += 1;
      return { body: { ...this.rowPage(r), has_children: false } };
    }
    const table = this.table(pageId);
    if (table === undefined) {
      return { status: 404 };
    }
    table.page = {
      ...table.page,
      title: text(body["title"]),
      meta_revision: table.page.meta_revision + 1,
    };
    return { body: table.page };
  }

  trash(pageId: string, trashed: boolean): Answer {
    const r = this.rowOf(pageId);
    if (r !== undefined) {
      r.trashed = trashed;
      return { body: { ...this.rowPage(r), has_children: false } };
    }
    const table = this.table(pageId);
    if (table === undefined) {
      return { status: 404 };
    }
    table.page = { ...table.page, trashed_at: trashed ? "2026-10-21T09:00:00Z" : null };
    return { body: table.page };
  }

  createTable(body: Record<string, unknown>): Answer {
    const tableId = this.made();
    const titleId = this.made();
    const viewId = this.made();
    const page = summary({
      id: tableId,
      title: text(body["title"]),
      parent: typeof body["parent"] === "string" ? body["parent"] : null,
      place: body["place"] === "workspace" ? "workspace" : "private",
      position: "a9",
    });
    const table: Table = {
      page,
      revision: 1,
      properties: [
        { id: titleId, name: "Name", type: "title", config: {}, position: "a0", can_revert: false },
      ],
      views: [{ id: viewId, name: "Table", type: "table", position: "a0", config: {} }],
      rows: [],
    };
    this.tables[tableId] = table;
    return { status: 201, body: this.detail(table) };
  }

  // -------------------------------------------------------------- routing

  /** The handler of "METHOD /path" (the path under the workspace), if the API has one. */
  handler(method: string, path: string): Handler | undefined {
    const match = (pattern: RegExp) => pattern.exec(path);
    const uuid = "([0-9a-f-]{36})";
    let found: RegExpExecArray | null;
    const run = (handler: Handler): Handler => {
      return (body, url) => {
        this.calls.push(`${method} ${path}`);
        return handler(body, url);
      };
    };
    if (method === "POST" && path === "/tables") {
      return run((body) => this.createTable(record(body)));
    }
    if ((found = match(new RegExp(`^/tables/${uuid}$`, "u"))) !== null && method === "GET") {
      const tableId = found[1] ?? "";
      return run(() => {
        const table = this.table(tableId);
        return table === undefined ? { status: 404 } : { body: this.detail(table) };
      });
    }
    if (
      (found = match(new RegExp(`^/tables/${uuid}/(query|rows|properties|views)$`, "u"))) !==
        null &&
      method === "POST"
    ) {
      const [, tableId = "", what = ""] = found;
      return run((body) => {
        switch (what) {
          case "query": {
            return this.query(tableId, record(body));
          }
          case "rows": {
            return this.addRow(tableId, record(body));
          }
          case "properties": {
            return this.addProperty(tableId, record(body));
          }
          default: {
            return this.createView(tableId, record(body));
          }
        }
      });
    }
    if (
      (found = match(
        new RegExp(`^/tables/${uuid}/properties/${uuid}(/options|/restore)?$`, "u"),
      )) !== null
    ) {
      const [, tableId = "", propertyId = "", suffix = ""] = found;
      if (suffix === "/options" && method === "PUT") {
        return run((body) => this.setOptions(tableId, propertyId, record(body)));
      }
      if (suffix === "/restore" && method === "POST") {
        return run(() => this.removeProperty(tableId, propertyId, false));
      }
      if (suffix === "" && method === "PATCH") {
        return run((body) => this.updateProperty(tableId, propertyId, record(body)));
      }
      if (suffix === "" && method === "DELETE") {
        return run(() => this.removeProperty(tableId, propertyId, true));
      }
    }
    if ((found = match(new RegExp(`^/tables/${uuid}/views/${uuid}$`, "u"))) !== null) {
      const [, tableId = "", viewId = ""] = found;
      if (method === "PATCH") {
        return run((body) => this.updateView(tableId, viewId, record(body)));
      }
      if (method === "DELETE") {
        return run(() => this.removeView(tableId, viewId));
      }
    }
    if (
      (found = match(new RegExp(`^/tables/${uuid}/rows/${uuid}/cells$`, "u"))) !== null &&
      method === "PATCH"
    ) {
      const [, tableId = "", rowId = ""] = found;
      return run((body) => this.setCells(tableId, rowId, record(body)));
    }
    if ((found = match(new RegExp(`^/pages/${uuid}(/trash|/restore|/sync)?$`, "u"))) !== null) {
      const [, pageId = "", suffix = ""] = found;
      if (suffix === "" && method === "GET") {
        return run(() => this.getPage(pageId));
      }
      if (suffix === "" && method === "PATCH") {
        return run((body) => this.rename(pageId, record(body)));
      }
      if (suffix === "/trash" && method === "POST") {
        return run(() => this.trash(pageId, true));
      }
      if (suffix === "/restore" && method === "POST") {
        return run(() => this.trash(pageId, false));
      }
      if (suffix === "/sync" && method === "POST") {
        return run((body) => this.sync(pageId, record(body)));
      }
    }
    return undefined;
  }

  private sync(pageId: string, body: Record<string, unknown>): Answer {
    const doc = this.docs.get(pageId) ?? new Y.Doc();
    this.docs.set(pageId, doc);
    if (typeof body["update"] === "string") {
      Y.applyUpdate(
        doc,
        Uint8Array.from(atob(body["update"]), (c) => c.codePointAt(0) ?? 0),
      );
    }
    const revision = this.rowOf(pageId)?.revision ?? this.table(pageId)?.revision ?? 1;
    return { body: { ...this.document(pageId), revision } };
  }

  /**
   * The routes, keyed "METHOD /path" under `base` (`/bff/api/v1/w/<workspace>`):
   * the `extra` ones first, then the table API for any table, row, property or
   * view that exists when it is called.
   */
  routes(base: string, extra: Record<string, Handler> = {}): Record<string, Handler> {
    return new Proxy(extra, {
      get: (target, key) => {
        if (typeof key !== "string") {
          return;
        }
        if (key in target) {
          return target[key];
        }
        const space = key.indexOf(" ");
        const path = key.slice(space + 1);
        return path.startsWith(base)
          ? this.handler(key.slice(0, space), path.slice(base.length))
          : undefined;
      },
    });
  }
}
