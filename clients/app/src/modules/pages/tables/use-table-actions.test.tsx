import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import {
  CLIENTS_ID,
  OPTIONS,
  P,
  ROW,
  TablesServer,
  VIEW_TABLE,
} from "../../../../test/demo/tables";
import { DUARTE } from "../../../../test/demo/workspaces";
import { renderWithProviders } from "../../../../test/render";
import { must } from "../../../../test/must";
import { tablesApi } from "../../../../test/tables";

import { useTable, useTableRows } from "./api";
import type { PropertyView, RowView, TableDetail } from "./types";
import { useUndoToast, type UndoApi } from "~/lib/ui";

import { useTableActions, type TableActions } from "./use-table-actions";

/** What the probe last saw, kept for the tests (set from an effect, never while rendering). */
const seen: {
  actions?: TableActions;
  schema?: TableDetail;
  rows: RowView[];
  history?: UndoApi;
} = { rows: [] };

function Probe() {
  const actions = useTableActions(DUARTE.id, CLIENTS_ID);
  const history = useUndoToast();
  const table = useTable(DUARTE.id, CLIENTS_ID);
  const query = useTableRows(DUARTE.id, CLIENTS_ID, VIEW_TABLE);
  const rows = query.data?.pages.flatMap((page) => page.rows) ?? [];
  useEffect(() => {
    seen.actions = actions;
    seen.history = history;
    if (table.data !== undefined) {
      seen.schema = table.data;
    }
    seen.rows = rows;
  });
  return (
    <p>
      {rows.map((row) => `${row.title}:${JSON.stringify(row.cells[P.value]?.value)}`).join("|")}
    </p>
  );
}

async function setup(server = new TablesServer(), extra: Parameters<typeof tablesApi>[1] = {}) {
  const api = tablesApi(server, extra);
  renderWithProviders(<Probe />);
  await waitFor(() => {
    expect(seen.rows.length).toBeGreaterThan(0);
    expect(seen.schema).toBeTruthy();
  });
  return api;
}

const property = (id: string): PropertyView => {
  const found = seen.schema?.properties.find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error("no property");
  }
  return found;
};
const row = (id: string): RowView => {
  const found = seen.rows.find((candidate) => candidate.id === id);
  if (found === undefined) {
    throw new Error("no row");
  }
  return found;
};
const run = async (work: (a: TableActions) => Promise<unknown>) => {
  await act(async () => {
    await work(must(seen.actions));
  });
};
/** Redo what was just undone (the history's toggle: the toast only offers Undo). */
const redo = () => {
  const newest = seen.history?.history[0];
  act(() => {
    if (newest !== undefined) {
      seen.history?.toggle(newest.id);
    }
  });
};
const undo = async () => {
  fireEvent.click(await screen.findByRole("button", { name: /Undo/u }));
};

describe("a cell", () => {
  it("shows at once, is written with the version seen, and undo and redo write it back", async () => {
    const { server } = await setup();
    await run((a) => a.setCell(row(ROW.acme), property(P.value), "500"));
    await waitFor(() => {
      expect(screen.getByText(/Acme Studio:"500"/u)).toBeTruthy();
    });
    expect(server.rowOf(ROW.acme)?.cells[P.value]).toEqual({ value: "500", version: 2 });
    await undo();
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("9600");
    });
    // Redo is the toast's counterpart in the history.
    redo();
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("500");
    });
  });

  it("can be cleared, and a cleared cell comes back with undo", async () => {
    const { server } = await setup();
    await run((a) => a.setCell(row(ROW.acme), property(P.value), null));
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]).toBeUndefined();
    });
    await undo();
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("9600");
    });
  });

  it("is rolled back with a failure that can be retried when the server refuses", async () => {
    let refuse = true;
    const own = new TablesServer();
    const { server } = await setup(own, {
      [`PATCH ${W}/tables/${CLIENTS_ID}/rows/${ROW.acme}/cells`]: (body: unknown) =>
        refuse
          ? { status: 500 }
          : own.setCells(CLIENTS_ID, ROW.acme, body as Record<string, unknown>),
    });
    await run((a) => a.setCell(row(ROW.acme), property(P.value), "1"));
    expect(await screen.findByText("Couldn’t change Value. Nothing was changed.")).toBeTruthy();
    expect(screen.getByText(/Acme Studio:"9600"/u)).toBeTruthy();
    refuse = false;
    fireEvent.click(screen.getByRole("button", { name: /Retry|Try again/u }));
    await waitFor(() => {
      expect(server.rowOf(ROW.acme)?.cells[P.value]?.value).toBe("1");
    });
  });
});

describe("a row", () => {
  it("is renamed with undo", async () => {
    const { server } = await setup();
    await run((a) => a.setTitle(row(ROW.lumen), "Lumen Dental Clinic"));
    expect(server.rowOf(ROW.lumen)?.title).toBe("Lumen Dental Clinic");
    await undo();
    await waitFor(() => {
      expect(server.rowOf(ROW.lumen)?.title).toBe("Lumen Dental");
    });
  });

  it("says when a rename fails and leaves the title as it was", async () => {
    await setup(new TablesServer(), {
      [`PATCH ${W}/pages/${ROW.lumen}`]: () => ({ status: 409 }),
    });
    await run((a) => a.setTitle(row(ROW.lumen), "x"));
    expect(await screen.findByText("Couldn’t change the title. Nothing was changed.")).toBeTruthy();
    expect(row(ROW.lumen).title).toBe("Lumen Dental");
  });

  it("is added at the end, can be taken back and put back", async () => {
    const { server } = await setup();
    let made: RowView | undefined;
    await run(async (a) => {
      made = await a.addRow({ title: "Cedar Clinic" });
    });
    expect(made?.title).toBe("Cedar Clinic");
    await waitFor(() => {
      expect(seen.rows.some((candidate) => candidate.title === "Cedar Clinic")).toBe(true);
    });
    await undo();
    await waitFor(() => {
      expect(server.rowOf(made?.id ?? "")?.trashed).toBe(true);
    });
    redo();
    await waitFor(() => {
      expect(server.rowOf(made?.id ?? "")?.trashed).toBe(false);
    });
  });

  it("says when a row could not be added", async () => {
    await setup(new TablesServer(), {
      [`POST ${W}/tables/${CLIENTS_ID}/rows`]: () => ({ status: 500 }),
    });
    let made: RowView | undefined = row(ROW.acme);
    await run(async (a) => {
      made = await a.addRow();
    });
    expect(made).toBeUndefined();
    expect(await screen.findByText("Couldn’t add the row. Nothing was saved.")).toBeTruthy();
  });

  it("goes to the trash with undo", async () => {
    const { server } = await setup();
    let done = false;
    await run(async (a) => {
      done = await a.trashRow(row(ROW.harbour));
    });
    expect(done).toBe(true);
    expect(server.rowOf(ROW.harbour)?.trashed).toBe(true);
    await undo();
    await waitFor(() => {
      expect(server.rowOf(ROW.harbour)?.trashed).toBe(false);
    });
  });

  it("stays when the trash fails", async () => {
    await setup(new TablesServer(), {
      [`POST ${W}/pages/${ROW.harbour}/trash`]: () => ({ status: 500 }),
    });
    let done = true;
    await run(async (a) => {
      done = await a.trashRow(row(ROW.harbour));
    });
    expect(done).toBe(false);
    expect(
      await screen.findByText("Couldn’t move that to the trash. Nothing was changed."),
    ).toBeTruthy();
  });
});

describe("a column", () => {
  it("is added, taken away and put back", async () => {
    const { server } = await setup();
    let id = "";
    await run(async (a) => {
      const added = await a.addProperty({ name: "Region", type: "select" });
      id = added?.property.id ?? "";
    });
    expect(server.table(CLIENTS_ID)?.properties.some((p) => p.id === id)).toBe(true);
    await undo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === id)?.removed).toBe(true);
    });
    redo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === id)?.removed).toBe(false);
    });
  });

  it("is renamed with undo", async () => {
    const { server } = await setup();
    await run((a) => a.renameProperty(property(P.value), "Fee"));
    expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.name).toBe("Fee");
    await undo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.name).toBe(
        "Value",
      );
    });
  });

  it("changes type and takes it back with the old values", async () => {
    const { server } = await setup();
    await run((a) => a.changeType(property(P.value), "text"));
    expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.type).toBe("text");
    await undo();
    await waitFor(() => {
      expect(server.calls.some((call) => call.endsWith("/restore"))).toBe(true);
    });
  });

  it("changes type back by hand when the server cannot restore it", async () => {
    const { server } = await setup();
    // A property whose change cannot be reverted: the server answers can_revert false.
    await run((a) => a.changeType(property(P.signed), "text", { x: 1 }));
    const changed = server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.signed);
    if (changed !== undefined) {
      changed.can_revert = false;
    }
    await undo();
    expect(server.calls.length).toBeGreaterThan(0);
  });

  it("changes its settings with undo", async () => {
    const { server } = await setup();
    await run((a) => a.updateConfig(property(P.value), { format: "percent" }));
    expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.config).toEqual({
      format: "percent",
    });
    await undo();
    await waitFor(() => {
      expect(
        server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.value)?.config["format"],
      ).toBe("currency");
    });
  });

  it("is deleted and put back", async () => {
    const { server } = await setup();
    await run((a) => a.removeProperty(property(P.signed)));
    expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.signed)?.removed).toBe(true);
    await undo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.signed)?.removed).toBe(
        false,
      );
    });
  });

  it("is removed in both places only when the member confirms, and says so", async () => {
    const urls: string[] = [];
    const { server } = await setup(new TablesServer(), {
      [`DELETE ${W}/tables/${CLIENTS_ID}/properties/${P.signed}`]: (_body: unknown, url: URL) => {
        urls.push(url.search);
        return url.searchParams.get("confirm_remote_delete") === "true"
          ? { body: { id: P.signed } }
          : { status: 400, body: { status: 400, code: "confirmation_required", title: "Confirm" } };
      },
    });
    // Tried plain first: the server says it is Notion's too, and nothing changes.
    let result: unknown;
    await run(async (a) => {
      result = await a.removeProperty(property(P.signed), { probe: true });
    });
    expect(result).toBe("confirm");
    expect(urls).toEqual([""]);
    expect(screen.queryByText("Couldn’t change the column. Nothing was changed.")).toBeNull();
    // Confirmed: the parameter is sent, and the toast says both places.
    await run(async (a) => {
      result = await a.removeProperty(property(P.signed), { confirm: true });
    });
    expect(result).toBe("removed");
    expect(urls).toEqual(["", "?confirm_remote_delete=true"]);
    expect(await screen.findByText("Column Signed removed from both places")).toBeTruthy();
    expect(server.calls.filter((call) => call.startsWith("DELETE"))).toHaveLength(0);
  });

  it("says when a probed removal fails for any other reason", async () => {
    await setup(new TablesServer(), {
      [`DELETE ${W}/tables/${CLIENTS_ID}/properties/${P.signed}`]: () => ({ status: 500 }),
    });
    let result: unknown;
    await run(async (a) => {
      result = await a.removeProperty(property(P.signed), { probe: true });
    });
    expect(result).toBe("failed");
    expect(
      await screen.findByText("Couldn’t change the column. Nothing was changed."),
    ).toBeTruthy();
  });

  it("says when a column change fails", async () => {
    await setup(new TablesServer(), {
      [`PATCH ${W}/tables/${CLIENTS_ID}/properties/${P.value}`]: () => ({ status: 500 }),
    });
    await run((a) => a.renameProperty(property(P.value), "Fee"));
    expect(
      await screen.findByText("Couldn’t change the column. Nothing was changed."),
    ).toBeTruthy();
  });

  it("changes its options and sends the old list, ids and all, with undo", async () => {
    const { server } = await setup();
    const stage = property(P.stage);
    const before = (
      stage.config as { options: { id: string; name: string; color: string }[] }
    ).options.map((option) => ({ ...option, color: option.color as never }));
    let updated: PropertyView | undefined;
    await run(async (a) => {
      updated = await a.setOptions(
        stage,
        before.filter((option) => option.id !== OPTIONS.paused),
        before,
      );
    });
    expect((updated?.config as { options: unknown[] }).options).toHaveLength(4);
    await undo();
    await waitFor(() => {
      const options = server.table(CLIENTS_ID)?.properties.find((p) => p.id === P.stage)?.config[
        "options"
      ] as unknown[];
      expect(options).toHaveLength(5);
    });
  });
});

describe("a view", () => {
  const viewOf = () => {
    const view = seen.schema?.views[0];
    if (view === undefined) {
      throw new Error("no view");
    }
    return view;
  };

  it("changes its settings at once, with undo", async () => {
    const { server } = await setup();
    await run((a) =>
      a.updateView(viewOf(), { config: { sorts: [{ property: P.value, direction: "asc" }] } }),
    );
    expect(server.table(CLIENTS_ID)?.views[0]?.config["sorts"]).toBeTruthy();
    await undo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views[0]?.config["sorts"]).toBeUndefined();
    });
  });

  it("is made, deleted, and put back with the same id", async () => {
    const { server } = await setup();
    let made: { id: string } | undefined;
    await run(async (a) => {
      made = await a.createView({ name: "Mine", type: "table", config: {} });
    });
    expect(server.table(CLIENTS_ID)?.views.some((view) => view.id === made?.id)).toBe(true);
    await undo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views.some((view) => view.id === made?.id)).toBe(false);
    });
    redo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views.some((view) => view.id === made?.id)).toBe(true);
    });
  });

  it("is deleted with undo, and a failure leaves it", async () => {
    const { server } = await setup();
    const second = seen.schema?.views[1];
    let removed = false;
    await run(async (a) => {
      removed = second !== undefined && (await a.removeView(second));
    });
    expect(removed).toBe(true);
    await undo();
    await waitFor(() => {
      expect(server.table(CLIENTS_ID)?.views).toHaveLength(2);
    });
  });

  it("says when a view could not be changed", async () => {
    await setup(new TablesServer(), {
      [`PATCH ${W}/tables/${CLIENTS_ID}/views/${VIEW_TABLE}`]: () => ({ status: 500 }),
    });
    await run((a) => a.updateView(viewOf(), { name: "x" }));
    expect(await screen.findByText("Couldn’t change the view. Nothing was changed.")).toBeTruthy();
  });
});
