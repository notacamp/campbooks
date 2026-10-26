import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ApiError } from "~/lib/api";

import { CONNECTION, NOTION_CONNECTION } from "../../../test/demo/identity";
import { NOTION_PAGES } from "../../../test/demo/pages";
import { DUARTE } from "../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../test/render";

import { notionListError, useNotionSource } from "./notion-source";

const W = `/bff/api/v1/w/${DUARTE.id}`;

function Probe({ id }: { id: string | undefined }) {
  const source = useNotionSource(DUARTE.id, id);
  return (
    <p>
      {source.connections?.map((c) => `${c.name}${c.needsReauth ? "!" : ""}`).join(",") ?? "-"}|
      {source.list.status}|{source.list.pages.length}|{String(source.list.truncated)}|
      {source.list.error ?? "-"}
    </p>
  );
}

describe("notionListError", () => {
  it("words each refusal by its stable code and status", () => {
    const fail = (status: number, code?: string) =>
      new ApiError(status, code === undefined ? undefined : { status, code, title: "x" });
    expect(notionListError(fail(409, "needs_reauth"))).toBe("needs_reauth");
    expect(notionListError(fail(429))).toBe("rate_limited");
    expect(notionListError(fail(403))).toBe("not_connected");
    expect(notionListError(fail(404))).toBe("not_connected");
    expect(notionListError(fail(503))).toBe("unavailable");
  });
});

describe("useNotionSource", () => {
  it("offers the caller's own Notion connections and the pages of the chosen one", async () => {
    mockApi({
      [`GET ${W}/connections`]: {
        body: {
          connections: [
            CONNECTION,
            NOTION_CONNECTION,
            { ...NOTION_CONNECTION, id: "other", granted_by_you: false, display_name: "Rui’s" },
          ],
        },
      },
      [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/pages`]: {
        body: { pages: NOTION_PAGES, truncated: true, requests: 1 },
      },
    });
    renderWithProviders(<Probe id={NOTION_CONNECTION.id} />);
    await waitFor(() => {
      expect(screen.getByText(`Acme|ready|${String(NOTION_PAGES.length)}|true|-`)).toBeVisible();
    });
  });

  it("says a connection that needs Notion again, and why a listing failed", async () => {
    mockApi({
      [`GET ${W}/connections`]: {
        body: { connections: [{ ...NOTION_CONNECTION, health: "needs_reauth" }] },
      },
      [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/pages`]: problem(409, "needs_reauth"),
    });
    renderWithProviders(<Probe id={NOTION_CONNECTION.id} />);
    await waitFor(() => {
      expect(screen.getByText("Acme!|error|0|false|needs_reauth")).toBeVisible();
    });
  });

  it("is idle with no connection chosen", async () => {
    mockApi({ [`GET ${W}/connections`]: { body: { connections: [NOTION_CONNECTION] } } });
    renderWithProviders(<Probe id={undefined} />);
    await waitFor(() => {
      expect(screen.getByText("Acme|idle|0|false|-")).toBeVisible();
    });
  });

  it("names a connection with no display name Notion", async () => {
    mockApi({
      [`GET ${W}/connections`]: {
        body: { connections: [{ ...NOTION_CONNECTION, display_name: null }] },
      },
    });
    renderWithProviders(<Probe id={undefined} />);
    await waitFor(() => {
      expect(screen.getByText("Notion|idle|0|false|-")).toBeVisible();
    });
  });
});
