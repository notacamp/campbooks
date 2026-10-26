import { screen, within } from "@testing-library/react";

import { MARTA_DETAIL } from "./demo/attention";
import { W } from "./demo/conversations";
import { TablesServer, type Handler } from "./demo/tables";
import { LEASE, TEAM } from "./demo/tasks";
import { mockApi, type MockHandler, type MockedApi } from "./render";

/**
 * The table API for a test: a server that keeps state, the workspace's
 * members, and the records the demo table links to. `extra` routes win.
 */
export function tablesApi(
  server = new TablesServer(),
  extra: Record<string, Handler | MockHandler> = {},
): { server: TablesServer; mocked: MockedApi } {
  const shell: Record<string, Handler> = {
    [`GET ${W}/pages`]: () => ({ body: { pages: server.pages(), tables: true } }),
    [`GET ${W}/pages/backlinks`]: () => ({ body: { pages: [] } }),
    [`GET ${W}/members`]: () => ({ body: { members: TEAM } }),
    [`GET ${W}/conversations/${MARTA_DETAIL.conversation.id}`]: () => ({ body: MARTA_DETAIL }),
    [`GET ${W}/tasks/${LEASE.id}`]: () => ({ body: { task: LEASE } }),
    [`GET ${W}/correspondents`]: () => ({ body: { correspondents: [] } }),
    [`GET ${W}/tasks`]: () => ({ body: { tasks: [] } }),
    [`GET ${W}/search`]: () => ({ body: { hits: [] } }),
  };
  const dynamic = server.routes(W, shell);
  const routes = new Proxy(
    {},
    {
      get: (_target, key) => {
        if (typeof key !== "string") {
          return;
        }
        const found = (key in extra ? extra[key] : undefined) ?? dynamic[key];
        return found === undefined
          ? undefined
          : (request: Request, body: unknown) =>
              (found as Handler)(body, new URL(request.url)) as ReturnType<
                Extract<MockHandler, (...args: never[]) => unknown>
              >;
      },
    },
  );
  return { server, mocked: mockApi(routes) };
}

/** The grid's cell at a row (by title) and column (by header name). */
export function cellAt(rowTitle: string, column: string): HTMLElement {
  const grid = screen.getByRole("grid");
  const headers = within(grid).getAllByRole("columnheader");
  const index = headers.findIndex((header) => header.getAttribute("aria-label") === column);
  const row = within(grid).getByRole("row", { name: rowTitle });
  const cell = within(row).getAllByRole("gridcell")[index];
  if (cell === undefined) {
    throw new Error(`no cell ${rowTitle} / ${column}`);
  }
  return cell;
}
