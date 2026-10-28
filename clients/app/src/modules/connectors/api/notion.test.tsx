import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ApiError } from "~/lib/api";

import { NOTION_CONNECTION } from "../../../../test/demo/identity";
import { NOTION_PAGES } from "../../../../test/demo/pages";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../test/render";

import { notionStartError, useNotionDataSources, useNotionPages, useStartNotion } from "./notion";

const W = `/bff/api/v1/w/${DUARTE.id}`;

function Start() {
  const start = useStartNotion(DUARTE.id);
  return (
    <button
      type="button"
      onClick={() => {
        start.mutate({ return_to: "/w/x/settings/mailboxes" });
      }}
    >
      {start.data?.authorize_url ?? (start.isError ? "refused" : "start")}
    </button>
  );
}

function Pages({ id }: { id: string | undefined }) {
  const pages = useNotionPages(DUARTE.id, id);
  return (
    <p>
      {pages.fetchStatus}|{pages.data?.pages.length ?? "-"}|{pages.isError ? "error" : "ok"}
    </p>
  );
}

function Databases({ id }: { id: string | undefined }) {
  const databases = useNotionDataSources(DUARTE.id, id);
  return (
    <p>
      {databases.fetchStatus}|
      {databases.data === undefined ? "-" : (databases.data?.data_sources.length ?? "none")}|
      {databases.isError ? "error" : "ok"}
    </p>
  );
}

describe("the Notion calls", () => {
  it("starts a consent and answers where the browser goes", async () => {
    const mocked = mockApi({
      [`POST ${W}/connections/notion/authorizations`]: {
        status: 201,
        body: {
          authorize_url: "/auth/start/notion?ticket=abc",
          expires_at: "2026-10-25T09:10:00Z",
        },
      },
    });
    renderWithProviders(<Start />);
    screen.getByRole("button").click();
    expect(await screen.findByText("/auth/start/notion?ticket=abc")).toBeVisible();
    expect(mocked.calls[0]?.body).toEqual({ return_to: "/w/x/settings/mailboxes" });
  });

  it("lists the pages of a connection, only once it is named", async () => {
    mockApi({
      [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/pages`]: {
        body: { pages: NOTION_PAGES, truncated: false, requests: 1 },
      },
    });
    const { rerender } = renderWithProviders(<Pages id={undefined} />);
    expect(await screen.findByText("idle|-|ok")).toBeVisible();
    rerender(<Pages id={NOTION_CONNECTION.id} />);
    await waitFor(() => {
      expect(screen.getByText(`idle|${String(NOTION_PAGES.length)}|ok`)).toBeVisible();
    });
  });

  it("lists the databases of a connection, only once it is named", async () => {
    mockApi({
      [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/data-sources`]: {
        body: {
          data_sources: [
            {
              data_source: "d1",
              title: "Clients",
              edited_at: "2026-10-27T09:00:00Z",
              columns: 7,
              linked: false,
            },
          ],
          truncated: false,
          requests: 1,
        },
      },
    });
    const { rerender } = renderWithProviders(<Databases id={undefined} />);
    expect(await screen.findByText("idle|-|ok")).toBeVisible();
    rerender(<Databases id={NOTION_CONNECTION.id} />);
    await waitFor(() => {
      expect(screen.getByText("idle|1|ok")).toBeVisible();
    });
  });

  it("takes a 404 for the databases as none to offer, not as a failure", async () => {
    mockApi({
      [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/data-sources`]: problem(
        404,
        "not_found",
      ),
    });
    renderWithProviders(<Databases id={NOTION_CONNECTION.id} />);
    await waitFor(() => {
      expect(screen.getByText("idle|none|ok")).toBeVisible();
    });
  });

  it("does not retry a refusal", async () => {
    const mocked = mockApi({
      [`GET ${W}/connections/${NOTION_CONNECTION.id}/notion/pages`]: problem(409, "needs_reauth"),
    });
    renderWithProviders(<Pages id={NOTION_CONNECTION.id} />);
    expect(await screen.findByText("idle|-|error")).toBeVisible();
    expect(mocked.calls).toHaveLength(1);
  });
});

describe("notionStartError", () => {
  const fail = (status: number, code?: string) =>
    new ApiError(status, code === undefined ? undefined : { status, code, title: "x" });

  it("tells an instance without Notion from a refusal of the person", () => {
    expect(notionStartError(fail(409, "provider_not_configured"))).toBe("unavailable");
    expect(notionStartError(fail(403, "connection_needs_user"))).toBe("not_member");
    expect(notionStartError(fail(403))).toBe("not_member");
    expect(notionStartError(fail(404))).toBe("not_member");
    expect(notionStartError(fail(429))).toBe("rate_limited");
    expect(notionStartError(fail(500))).toBe("failed");
    expect(notionStartError(new TypeError("offline"))).toBe("failed");
  });
});
