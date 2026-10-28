import { describe, expect, it } from "vitest";

import { W } from "./conversations";
import { NOTION_CONNECTION } from "./identity";
import { CELL_CONFLICT, COLUMN_CONFLICT, NotionTablesServer } from "./notion-tables";
import { CLIENTS_ID, P, ROW } from "./tables";
import { tablesApi } from "../tables";

// The fake the stories and the screens' tests share: what it answers is what
// the pages and connectors operations answer.
const url = (path: string) => `http://localhost${W}${path}`;
const get = async (path: string) => (await fetch(url(path))).json() as Promise<unknown>;

describe("the fake Notion tables server", () => {
  it("offers the databases, and makes the table when the import is made", async () => {
    const server = new NotionTablesServer();
    server.makeSynced({ hidden: true });
    tablesApi(server);
    const listed = (await get(`/connections/${NOTION_CONNECTION.id}/notion/data-sources`)) as {
      data_sources: { title: string; linked: boolean }[];
    };
    expect(listed.data_sources.map((database) => [database.title, database.linked])).toEqual([
      ["Clients", false],
      ["Projects", false],
    ]);
    expect(server.pages().some((page) => page.id === CLIENTS_ID)).toBe(false);
    const made = await fetch(url(`/pages/notion/imports`), { method: "POST", body: "{}" });
    expect(made.status).toBe(201);
    expect(server.pages().some((page) => page.id === CLIENTS_ID)).toBe(true);
  });

  it("raises the conflicts it is asked for, settles them and takes the choice back", async () => {
    const server = new NotionTablesServer();
    server.makeSynced({ cellConflict: true, columnConflict: true });
    tablesApi(server);
    const before = (await get(`/tables/${CLIENTS_ID}/notion`)) as {
      link: { conflicts: number };
      conflicts: { id: string }[];
    };
    expect(before.link.conflicts).toBe(2);
    expect(before.conflicts.map((conflict) => conflict.id)).toEqual([
      CELL_CONFLICT,
      COLUMN_CONFLICT,
    ]);
    const settle = (id: string, keep: string, undo = false) =>
      fetch(url(`/tables/${CLIENTS_ID}/notion/conflicts/${id}${undo ? "/undo" : ""}`), {
        method: "POST",
        body: JSON.stringify({ keep }),
      });
    expect((await settle(CELL_CONFLICT, "theirs")).status).toBe(200);
    expect(server.rowOf(ROW.northwind)?.cells[P.stage]?.value).toBe("o-proposal");
    expect((await settle(CELL_CONFLICT, "theirs", true)).status).toBe(200);
    expect(server.rowOf(ROW.northwind)?.cells[P.stage]?.value).toBe("o-kickoff");
    expect(server.conflicts).toHaveLength(2);
    expect((await settle(COLUMN_CONFLICT, "theirs")).status).toBe(200);
    expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.name).toBe(
      "Contract value",
    );
    // Nothing is left to settle twice.
    expect((await settle(COLUMN_CONFLICT, "ours")).status).toBe(409);
    expect((await settle("0192f1a0-0000-7000-8000-00000000ffff", "ours", true)).status).toBe(409);
  });

  it("stops syncing, and refuses to stop what is not syncing", async () => {
    const server = new NotionTablesServer();
    tablesApi(server);
    const stop = () => fetch(url(`/tables/${CLIENTS_ID}/notion/stop`), { method: "POST" });
    expect((await stop()).status).toBe(409);
    server.makeSynced();
    expect((await stop()).status).toBe(200);
    expect(server.link?.state).toBe("stopped");
  });
});
