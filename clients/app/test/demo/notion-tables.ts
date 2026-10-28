// A Notion database imported as a table (campbooks-cloud#377), for tests and
// stories: the demo clients table made Notion's (a people column, a formula,
// a relation kept in Notion, a Campbooks-only column), its sync and the
// conflicts on it, the databases the import lists, and the answers of the
// calls a member makes on them. Fictional people only.
import type { Schema } from "~/lib/api";

import { CLIENTS_ID, OPTIONS, P, ROW, TablesServer, type Answer, type Handler } from "./tables";
import { NOTION_CONNECTION } from "./identity";
import { ANA, RUI } from "./workspaces";

type Link = Schema<"NotionTableLink">;
type Conflict = Schema<"TableConflict">;

const id = (n: number) => `0192f1a0-0000-7000-8000-00000000${n.toString(16).padStart(4, "0")}`;

/** The Notion connection and the database the clients table is kept in step with. */
const SYNC_CONNECTION = NOTION_CONNECTION.id;
const CLIENTS_DATA_SOURCE = id(36_865);
const PROJECTS_DATA_SOURCE = id(36_866);

/** The column a Notion formula computes (Score) and the conflicts the demo raises. */
export const SCORE = id(36_881);
export const CELL_CONFLICT = id(36_897);
export const COLUMN_CONFLICT = id(36_898);

/** The databases the import lists. */
export const NOTION_DATABASES: readonly Schema<"NotionDataSource">[] = [
  {
    data_source: CLIENTS_DATA_SOURCE,
    title: "Clients",
    edited_at: "2026-10-27T09:00:00Z",
    columns: 7,
    linked: false,
  },
  {
    data_source: PROJECTS_DATA_SOURCE,
    title: "Projects",
    edited_at: "2026-10-26T09:00:00Z",
    columns: 5,
    linked: false,
  },
];

const problem = (status: number, code: string, title: string): Answer => ({
  status,
  body: { status, code, title },
});

export interface SyncedOptions {
  /** Northwind's Stage was changed here and in Notion. */
  cellConflict?: boolean;
  /** Value was renamed here and in Notion. */
  columnConflict?: boolean;
  /** The table is not in the tree until the import makes it. */
  hidden?: boolean;
}

/** The clients table, kept in step with a Notion database. */
export class NotionTablesServer extends TablesServer {
  link: Link | null = null;
  conflicts: Conflict[] = [];
  /** Conflicts settled, by id, so Undo can put them back. */
  private settled = new Map<
    string,
    { conflict: Conflict; keep: "ours" | "theirs"; before: unknown }
  >();
  /** Columns removed with the member's confirmation. */
  confirmedRemovals: string[] = [];
  private imported = true;

  /** Make the clients table Notion's, as the import leaves it. */
  makeSynced(options: SyncedOptions = {}) {
    const table = this.table(CLIENTS_ID);
    if (table === undefined) {
      return;
    }
    const owner = table.properties.find((property) => property.id === P.owner);
    if (owner !== undefined) {
      owner.type = "notion_people";
      owner.config = {};
    }
    table.properties = table.properties.filter(
      (property) => property.id !== P.next && property.id !== P.signed,
    );
    const projects = table.properties.find((property) => property.id === P.projects);
    if (projects !== undefined) {
      projects.type = "external";
      projects.config = {};
      projects.name = "Projects";
    }
    table.properties.splice(5, 0, {
      id: SCORE,
      name: "Score",
      type: "notion_formula",
      config: {},
      position: "a45",
      can_revert: false,
    });
    const names = new Map<string, string>([
      [ROW.acme, ANA.display_name],
      [ROW.lumen, RUI.display_name],
      [ROW.northwind, ANA.display_name],
      [ROW.harbour, RUI.display_name],
      [ROW.oldmill, RUI.display_name],
    ]);
    const scores = new Map<string, string>([
      [ROW.acme, "82"],
      [ROW.lumen, "91"],
      [ROW.northwind, "64"],
      [ROW.harbour, "40"],
      [ROW.oldmill, "12"],
    ]);
    for (const row of table.rows) {
      row.cells[P.owner] = { value: names.get(row.id) ?? "", version: 1 };
      row.cells[SCORE] = { value: scores.get(row.id) ?? "", version: 1 };
      row.cells[P.projects] = { value: row.id === ROW.acme ? "2 projects" : "", version: 1 };
      row.cells[P.stage] = {
        value: row.id === ROW.northwind ? OPTIONS.kickoff : row.cells[P.stage]?.value,
        version: 1,
      };
    }
    for (const view of table.views) {
      view.config = {
        ...view.config,
        summaries: [{ property: P.value, kind: "sum" }],
      };
    }
    this.link = {
      connection: SYNC_CONNECTION,
      data_source: CLIENTS_DATA_SOURCE,
      state: "syncing",
      stopped_reason: null,
      owner: ANA.user_id,
      conflicts: 0,
      last_inbound_at: new Date(Date.now() - 60_000).toISOString(),
      last_outbound_at: null,
    };
    this.conflicts = [];
    if (options.cellConflict === true) {
      this.conflicts.push({
        id: CELL_CONFLICT,
        kind: "cell",
        property: P.stage,
        row: ROW.northwind,
        ours: OPTIONS.kickoff,
        theirs: OPTIONS.proposal,
        ours_at: "2026-10-27T10:30:00Z",
        theirs_at: "2026-10-27T10:42:00Z",
      });
    }
    if (options.columnConflict === true) {
      const value = table.properties.find((property) => property.id === P.value);
      if (value !== undefined) {
        value.name = "Deal size";
        this.conflicts.push({
          id: COLUMN_CONFLICT,
          kind: "column",
          property: P.value,
          row: null,
          ours: { name: "Deal size", type: "number", config: value.config },
          theirs: { name: "Contract value", type: "number", config: value.config },
          ours_at: "2026-10-27T10:30:00Z",
          theirs_at: "2026-10-27T10:42:00Z",
        });
      }
    }
    this.imported = options.hidden !== true;
    this.refreshCount();
  }

  override reset() {
    super.reset();
    this.link = null;
    this.conflicts = [];
    this.settled.clear();
    this.confirmedRemovals = [];
    this.imported = true;
  }

  override pages(extra: readonly Schema<"PageSummary">[] = []) {
    const all = super.pages(extra);
    return this.imported ? all : all.filter((page) => page.id !== CLIENTS_ID);
  }

  private refreshCount() {
    if (this.link !== null) {
      this.link = { ...this.link, conflicts: this.conflicts.length };
    }
  }

  private body() {
    return { link: this.link, conflicts: this.conflicts };
  }

  private resolve(conflictId: string, keep: "ours" | "theirs"): Answer {
    const conflict = this.conflicts.find((candidate) => candidate.id === conflictId);
    const table = this.table(CLIENTS_ID);
    if (conflict === undefined || table === undefined) {
      return problem(409, "no_conflict", "No conflict");
    }
    const property = table.properties.find((candidate) => candidate.id === conflict.property);
    let before: unknown;
    if (conflict.kind === "cell") {
      const cell = table.rows.find((row) => row.id === conflict.row)?.cells[conflict.property];
      before = cell?.value;
      if (keep === "theirs" && cell !== undefined) {
        cell.value = conflict.theirs;
        cell.version += 1;
      }
    } else if (property !== undefined) {
      before = property.name;
      if (keep === "theirs") {
        property.name = (conflict.theirs as { name: string }).name;
      }
    }
    this.conflicts = this.conflicts.filter((candidate) => candidate.id !== conflictId);
    this.settled.set(conflictId, { conflict, keep, before });
    this.refreshCount();
    return { body: { conflict: conflictId, conflicts: this.conflicts.length } };
  }

  private undo(conflictId: string): Answer {
    const found = this.settled.get(conflictId);
    const table = this.table(CLIENTS_ID);
    if (found === undefined || table === undefined) {
      return problem(409, "cannot_undo", "Cannot undo");
    }
    const { conflict, before } = found;
    if (conflict.kind === "cell") {
      const cell = table.rows.find((row) => row.id === conflict.row)?.cells[conflict.property];
      if (cell !== undefined) {
        cell.value = before;
      }
    } else {
      const property = table.properties.find((candidate) => candidate.id === conflict.property);
      if (property !== undefined) {
        property.name = String(before);
      }
    }
    this.settled.delete(conflictId);
    this.conflicts.push(conflict);
    this.refreshCount();
    return { body: { conflicts: this.conflicts.length } };
  }

  /** The Notion routes first, then the table API. */
  override handler(method: string, path: string): Handler | undefined {
    const uuid = "([0-9a-f-]{36})";
    const log = (handler: Handler): Handler => {
      return (body, url) => {
        this.calls.push(`${method} ${path}`);
        return handler(body, url);
      };
    };
    let found: RegExpExecArray | null;
    if (method === "GET" && path === `/tables/${CLIENTS_ID}/notion`) {
      return log(() => ({ body: this.body() }));
    }
    if (
      (found = new RegExp(`^/tables/${uuid}/notion/conflicts/${uuid}(/undo)?$`, "u").exec(path)) !==
        null &&
      method === "POST"
    ) {
      const conflictId = found[2] ?? "";
      return found[3] === undefined
        ? log((body) => {
            const keep = (body as { keep?: string }).keep;
            return this.resolve(conflictId, keep === "theirs" ? "theirs" : "ours");
          })
        : log(() => this.undo(conflictId));
    }
    if (method === "POST" && path === `/tables/${CLIENTS_ID}/notion/stop`) {
      return log(() => {
        if (this.link === null) {
          return problem(409, "not_syncing", "Not syncing");
        }
        this.link = { ...this.link, state: "stopped", stopped_reason: "stopped_by_member" };
        return { body: this.link };
      });
    }
    if (method === "DELETE") {
      const removal = new RegExp(`^/tables/${uuid}/properties/${uuid}$`, "u").exec(path);
      const inner = super.handler(method, path);
      if (removal !== null && inner !== undefined) {
        const propertyId = removal[2] ?? "";
        return (body, url) => {
          const property = this.table(CLIENTS_ID)?.properties.find(
            (candidate) => candidate.id === propertyId,
          );
          // Notion's own column of a table that is syncing: only with the member's say-so.
          const notions =
            this.link?.state === "syncing" &&
            property !== undefined &&
            property.type !== "campbooks_link" &&
            property.type !== "person";
          const confirmed = url.searchParams.get("confirm_remote_delete") === "true";
          if (notions && !confirmed) {
            this.calls.push(`${method} ${path} (refused)`);
            return problem(400, "confirmation_required", "Confirmation required");
          }
          if (confirmed) {
            this.confirmedRemovals.push(propertyId);
          }
          return inner(body, url);
        };
      }
    }
    return super.handler(method, path);
  }

  /** The routes under the workspace, with the import's: `extra` wins. */
  override routes(base: string, extra: Record<string, Handler> = {}): Record<string, Handler> {
    const notion: Record<string, Handler> = {
      [`GET ${base}/connections/${SYNC_CONNECTION}/notion/data-sources`]: () => ({
        body: {
          data_sources: NOTION_DATABASES.map((database) => ({
            ...database,
            linked:
              this.imported && this.link !== null && database.data_source === CLIENTS_DATA_SOURCE,
          })),
          truncated: false,
          requests: 1,
        },
      }),
      [`GET ${base}/connections/${SYNC_CONNECTION}/notion/pages`]: () => ({
        body: { pages: [], truncated: false, requests: 1 },
      }),
      [`POST ${base}/pages/notion/imports`]: () => {
        this.imported = true;
        return {
          status: 201,
          body: {
            pages: [],
            skipped: 0,
            tables: [{ data_source: CLIENTS_DATA_SOURCE, table: CLIENTS_ID }],
          },
        };
      },
      [`GET ${base}/pages/notion/links`]: () => ({ body: { links: [] } }),
    };
    return super.routes(base, { ...notion, ...extra });
  }
}
