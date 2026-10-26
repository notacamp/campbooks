import { waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import {
  CLIENTS_ID,
  P,
  PROJECT_ROW,
  PROJECTS_ID,
  ROW,
  TablesServer,
} from "../../../../test/demo/tables";
import { DUARTE } from "../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../test/render";
import { tablesApi } from "../../../../test/tables";

import { useRowsById, useTable, useTableCalls, useTableRows } from "./api";

import type { ReactNode } from "react";

// A probe component stands in for a hook renderer: it needs the providers the screens have.
function probe<TResult>(use: () => TResult) {
  const seen: { current: TResult | undefined } = { current: undefined };
  function Probe(): ReactNode {
    seen.current = use();
    return null;
  }
  const view = renderWithProviders(<Probe />);
  return { seen, ...view };
}

describe("reading a table", () => {
  it("reads its schema and the rows of a view, 100 at a time", async () => {
    tablesApi();
    const { seen } = probe(() => ({
      table: useTable(DUARTE.id, CLIENTS_ID),
      rows: useTableRows(DUARTE.id, CLIENTS_ID),
    }));
    await waitFor(() => {
      expect(seen.current?.table.data?.properties.length).toBe(8);
    });
    // Without a view there is nothing to ask for yet.
    expect(seen.current?.rows.fetchStatus).toBe("idle");
  });

  it("reads the rows of a table by id, only the table's own", async () => {
    const { server } = tablesApi();
    const { seen } = probe(() =>
      useRowsById(DUARTE.id, PROJECTS_ID, [PROJECT_ROW, ROW.acme, PROJECT_ROW]),
    );
    await waitFor(() => {
      expect(seen.current?.data?.rows.map((row) => row.title)).toEqual(["Acme rebrand"]);
    });
    expect(server.calls.filter((call) => call.endsWith("/query"))).toHaveLength(1);
  });

  it("does not ask for rows when it has none to ask for", () => {
    const { server } = tablesApi();
    const { seen } = probe(() => useRowsById(DUARTE.id, PROJECTS_ID, []));
    expect(seen.current?.fetchStatus).toBe("idle");
    expect(server.calls).toHaveLength(0);
  });

  it("gives up at once on a table it may not open", async () => {
    tablesApi(new TablesServer(), { [`GET ${W}/tables/${CLIENTS_ID}`]: () => ({ status: 404 }) });
    const { seen } = probe(() => useTable(DUARTE.id, CLIENTS_ID));
    await waitFor(() => {
      expect(seen.current?.isError).toBe(true);
    });
  });
});

describe("the calls", () => {
  it("make a table, read it, and query with a filter of their own", async () => {
    const { server } = tablesApi();
    const { seen } = probe(() => useTableCalls(DUARTE.id));
    const calls = seen.current;
    if (calls === undefined) {
      throw new Error("no calls");
    }
    const made = await calls.createTable({ title: "Leads", place: "workspace" });
    expect(made.page.title).toBe("Leads");
    expect((await calls.getTable(made.page.id)).views).toHaveLength(1);
    const found = await calls.query(CLIENTS_ID, {
      filter: { and: [{ property: P.value, operator: "gt", value: "5000" }] },
    });
    expect(found.rows.map((row) => row.title)).toEqual(["Acme Studio", "Lumen Dental"]);
    expect(server.calls.length).toBeGreaterThan(2);
  });
});
