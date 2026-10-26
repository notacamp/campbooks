import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, NOTION_ROADMAP, notionLink } from "../../../../test/demo/pages";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../test/render";

import { useNotionCalls, useNotionLink, useNotionLinks, useRefreshNotion } from "./notion";

const LINKS = `GET ${W}/pages/notion/links`;

function Probe() {
  const all = useNotionLinks(DUARTE.id);
  const one = useNotionLink(DUARTE.id, HANDBOOK.id);
  return (
    <p>
      {all.isSuccess ? String(all.data.length) : "-"}|{one?.state ?? "none"}
    </p>
  );
}

describe("the Notion links", () => {
  it("lists the links and finds a page's own", async () => {
    mockApi({
      [LINKS]: { body: { links: [notionLink(HANDBOOK.id, NOTION_ROADMAP, { conflicts: 1 })] } },
    });
    renderWithProviders(<Probe />);
    expect(await screen.findByText("1|syncing")).toBeVisible();
  });

  it("reads a server without Notion as no links", async () => {
    mockApi({ [LINKS]: { status: 404, body: { status: 404, title: "Not found" } } });
    renderWithProviders(<Probe />);
    expect(await screen.findByText("0|none")).toBeVisible();
  });

  it("is an error when the server fails", async () => {
    mockApi({ [LINKS]: problem(500, "internal") });
    renderWithProviders(<Probe />);
    await waitFor(() => {
      expect(screen.getByText("-|none")).toBeVisible();
    });
  });
});

function Calls() {
  const calls = useNotionCalls(DUARTE.id);
  const refresh = useRefreshNotion(DUARTE.id);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          void calls.import({ connection: "c", pages: [{ notion_page: "n" }] });
          void calls.resolve("p", "b", "both");
          void calls.undo("p", "r");
          void calls.stop("p");
          refresh("p");
          refresh();
        }}
      >
        go
      </button>
    </>
  );
}

describe("the Notion calls", () => {
  it("sends each call to its own address", async () => {
    const base = `${W}/pages`;
    const mocked = mockApi({
      [`POST ${base}/notion/imports`]: { status: 201, body: { pages: [], skipped: 0 } },
      [`POST ${base}/p/notion/conflicts/b`]: {
        body: { resolution: "r", revision: 2, conflicts: 0 },
      },
      [`POST ${base}/p/notion/resolutions/r/undo`]: { body: { revision: 3, conflicts: 1 } },
      [`POST ${base}/p/notion/stop`]: { body: notionLink("p", "n", { state: "stopped" }) },
    });
    renderWithProviders(<Calls />);
    screen.getByRole("button", { name: "go" }).click();
    await waitFor(() => {
      expect(mocked.calls.map((call) => call.route).toSorted()).toEqual(
        [
          `POST ${base}/notion/imports`,
          `POST ${base}/p/notion/conflicts/b`,
          `POST ${base}/p/notion/resolutions/r/undo`,
          `POST ${base}/p/notion/stop`,
        ].toSorted(),
      );
    });
    expect(mocked.calls.find((c) => c.route.endsWith("/conflicts/b"))?.body).toEqual({ keep: "both" });
    expect(mocked.calls.find((c) => c.route.endsWith("/imports"))?.body).toEqual({
      connection: "c",
      pages: [{ notion_page: "n" }],
    });
  });
});
