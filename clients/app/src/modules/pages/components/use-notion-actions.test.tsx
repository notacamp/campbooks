import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { DUARTE } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";

import { useNotionActions } from "./use-notion-actions";

const P = `${W}/pages/p1`;

function Buttons() {
  const notion = useNotionActions(DUARTE.id);
  return (
    <>
      <button type="button" onClick={() => void notion.resolve("p1", "b1", "both")}>
        resolve
      </button>
      <button type="button" onClick={() => void notion.stop("p1")}>
        stop
      </button>
      <button
        type="button"
        onClick={() =>
          void notion
            .importPages({ connection: "c1", pages: [{ notion_page: "n1" }] })
            .then((r) => {
              document.title = r === undefined ? "failed" : `made ${String(r.pages.length)}`;
            })
        }
      >
        import
      </button>
    </>
  );
}

describe("useNotionActions", () => {
  it("settles a conflict, says it goes to Notion next sync, and takes it back with Undo", async () => {
    const mocked = mockApi({
      [`POST ${P}/notion/conflicts/b1`]: { body: { resolution: "r1", revision: 4, conflicts: 0 } },
      [`POST ${P}/notion/resolutions/r1/undo`]: { body: { revision: 5, conflicts: 1 } },
      [`GET ${W}/pages/notion/links`]: { body: { links: [] } },
    });
    renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "resolve" }));

    expect(
      await screen.findByText(
        "Kept both, one under the other — it goes to Notion on the next sync.",
      ),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Undo/u }));
    await waitFor(() => {
      expect(
        mocked.calls.some((call) => call.route === `POST ${P}/notion/resolutions/r1/undo`),
      ).toBe(true);
    });
  });

  it("says a choice could not be applied, and tries again from the toast", async () => {
    let tries = 0;
    mockApi({
      [`POST ${P}/notion/conflicts/b1`]: () => {
        tries += 1;
        return tries === 1
          ? { status: 500, body: { status: 500, title: "x" } }
          : { body: { resolution: "r1", revision: 4, conflicts: 0 } };
      },
    });
    renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "resolve" }));
    expect(
      await screen.findByText("Couldn’t apply that choice. Nothing was changed."),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Retry|Try again/u }));
    await waitFor(() => {
      expect(tries).toBe(2);
    });
  });

  it("says when an Undo failed", async () => {
    mockApi({
      [`POST ${P}/notion/conflicts/b1`]: { body: { resolution: "r1", revision: 4, conflicts: 0 } },
      [`POST ${P}/notion/resolutions/r1/undo`]: {
        status: 409,
        body: { status: 409, code: "cannot_undo", title: "x" },
      },
    });
    renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "resolve" }));
    fireEvent.click(await screen.findByRole("button", { name: /Undo/u }));
    expect(
      await screen.findByText("Couldn’t undo that choice. Nothing was changed."),
    ).toBeVisible();
  });

  it("stops syncing and says so, or says it could not", async () => {
    mockApi({
      [`POST ${P}/notion/stop`]: { body: { page: "p1" } },
    });
    const { unmount } = renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "stop" }));
    expect(await screen.findByText("Stopped syncing with Notion")).toBeVisible();
    unmount();

    mockApi({ [`POST ${P}/notion/stop`]: { status: 409, body: { status: 409, title: "x" } } });
    renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "stop" }));
    expect(await screen.findByText("Couldn’t stop syncing. Nothing was changed.")).toBeVisible();
  });

  it("imports pages and says how many, or answers nothing when it failed", async () => {
    mockApi({
      [`POST ${W}/pages/notion/imports`]: {
        status: 201,
        body: { pages: [{ page: "a", notion_page: "n1" }], skipped: 0 },
      },
      [`GET ${W}/pages`]: { body: { pages: [] } },
    });
    const { unmount } = renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "import" }));
    expect(await screen.findByText("Imported 1 page from Notion")).toBeVisible();
    await waitFor(() => {
      expect(document.title).toBe("made 1");
    });
    unmount();

    mockApi({
      [`POST ${W}/pages/notion/imports`]: { status: 500, body: { status: 500, title: "x" } },
    });
    renderWithProviders(<Buttons />);
    fireEvent.click(screen.getByRole("button", { name: "import" }));
    await waitFor(() => {
      expect(document.title).toBe("failed");
    });
  });
});
