import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, OLD_NOTES, PAGES } from "../../../../test/demo/pages";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";

import { useBacklinks, usePagesAvailable, useTrash } from "./queries";

function Probe() {
  const available = usePagesAvailable(DUARTE.id);
  const trash = useTrash(DUARTE.id, true);
  const links = useBacklinks(DUARTE.id, "tasks.task", "k1");
  return (
    <p>
      {String(available)}|{trash.data?.map((page) => page.title).join(",") ?? "-"}|
      {links.data?.map((link) => link.title).join(",") ?? "-"}
    </p>
  );
}

describe("the pages queries", () => {
  it("says pages are available once the list answers, and reads the trash and backlinks", async () => {
    mockApi({
      [`GET ${W}/pages`]: (request) => ({
        body: {
          pages: new URL(request.url).searchParams.get("trashed") === "true" ? [OLD_NOTES] : PAGES,
        },
      }),
      [`GET ${W}/pages/backlinks`]: {
        body: {
          pages: [
            { page: HANDBOOK.id, title: "Team handbook", place: "workspace", edited_at: "x" },
          ],
        },
      },
    });
    renderWithProviders(<Probe />);
    expect(await screen.findByText("true|Old meeting notes|Team handbook")).toBeTruthy();
  });

  it("says pages are not available when the server has none (the module is off)", async () => {
    mockApi({
      [`GET ${W}/pages`]: { status: 404, body: { status: 404, title: "Not found" } },
      [`GET ${W}/pages/backlinks`]: { status: 404, body: { status: 404, title: "Not found" } },
    });
    renderWithProviders(<Probe />);
    expect(await screen.findByText("false|-|-")).toBeTruthy();
  });
});
