import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, IDEAS, plainDoc, viewOfDoc } from "../../../../test/demo/pages";
import { DUARTE, RUI } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders, type MockHandler } from "../../../../test/render";

import { usePageActions } from "./use-page-actions";

const P = `${W}/pages`;
const made = viewOfDoc(IDEAS, plainDoc());

type Actions = ReturnType<typeof usePageActions>;

/** Buttons that run each action with a page, for the test to press. */
function Harness() {
  const actions: Actions = usePageActions(DUARTE.id);
  const to = { parent: HANDBOOK.id, place: "workspace" as const, position: "a5" };
  return (
    <div>
      <button
        onClick={() => {
          void actions.create({ place: "private" });
        }}
      >
        create
      </button>
      <button
        onClick={() => {
          void actions.trash(IDEAS);
        }}
      >
        trash
      </button>
      <button
        onClick={() => {
          void actions.restore(IDEAS);
        }}
      >
        restore
      </button>
      <button
        onClick={() => {
          void actions.purge(IDEAS);
        }}
      >
        purge
      </button>
      <button
        onClick={() => {
          void actions.move(IDEAS, to);
        }}
      >
        move
      </button>
      <button
        onClick={() => {
          void actions.share(IDEAS, RUI.user_id, "writer", undefined, "Rui Matos");
        }}
      >
        share
      </button>
      <button
        onClick={() => {
          void actions.share(IDEAS, RUI.user_id, "writer", "reader", "Rui Matos");
        }}
      >
        reshare
      </button>
      <button
        onClick={() => {
          void actions.unshare(IDEAS, RUI.user_id, "reader", "Rui Matos");
        }}
      >
        unshare
      </button>
      <button
        onClick={() => {
          void actions.rename(IDEAS, { title: "Plan", icon: null });
        }}
      >
        rename
      </button>
    </div>
  );
}

const routes = (extra: Record<string, MockHandler> = {}): Record<string, MockHandler> => ({
  [`POST ${P}`]: { status: 201, body: made },
  [`POST ${P}/${IDEAS.id}/trash`]: { body: { ...IDEAS, trashed_at: "2026-10-13T09:00:00Z" } },
  [`POST ${P}/${IDEAS.id}/restore`]: { body: IDEAS },
  [`DELETE ${P}/${IDEAS.id}`]: { status: 204 },
  [`POST ${P}/${IDEAS.id}/move`]: { body: { ...IDEAS, meta_revision: 2 } },
  [`PUT ${P}/${IDEAS.id}/grants/${RUI.user_id}`]: {
    body: { member: RUI.user_id, role: "writer" },
  },
  [`DELETE ${P}/${IDEAS.id}/grants/${RUI.user_id}`]: { status: 204 },
  [`PATCH ${P}/${IDEAS.id}`]: { body: { ...IDEAS, title: "Plan" } },
  ...extra,
});

const press = (name: string) => {
  fireEvent.click(screen.getByRole("button", { name }));
};
const undo = async () => {
  fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
};
const called = (api: ReturnType<typeof mockApi>, route: string) =>
  api.calls.filter((call) => call.route === route);

describe("a member's changes to pages", () => {
  it("makes a page, and Undo puts it in the trash", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("create");
    await undo();
    await waitFor(() => {
      expect(called(api, `POST ${P}/${IDEAS.id}/trash`)).toHaveLength(1);
    });
    expect(called(api, `POST ${P}`)[0]?.body).toEqual({ place: "private" });
  });

  it("trashes a page, and Undo brings it back", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("trash");
    await undo();
    await waitFor(() => {
      expect(called(api, `POST ${P}/${IDEAS.id}/restore`)).toHaveLength(1);
    });
  });

  it("restores a page, and Undo trashes it again", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("restore");
    await undo();
    await waitFor(() => {
      expect(called(api, `POST ${P}/${IDEAS.id}/trash`)).toHaveLength(1);
    });
  });

  it("deletes for good and says so", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("purge");
    expect(await screen.findByText(/deleted for good/u)).toBeTruthy();
    expect(called(api, `DELETE ${P}/${IDEAS.id}`)).toHaveLength(1);
  });

  it("moves a page, and Undo moves it back to where it was", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("move");
    await undo();
    await waitFor(() => {
      expect(called(api, `POST ${P}/${IDEAS.id}/move`)).toHaveLength(2);
    });
    const [first, second] = called(api, `POST ${P}/${IDEAS.id}/move`);
    expect(first?.body).toMatchObject({ parent: HANDBOOK.id, place: "workspace" });
    expect(second?.body).toMatchObject({ parent: null, place: IDEAS.place });
  });

  it("shares a page, and Undo stops sharing it", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("share");
    await undo();
    await waitFor(() => {
      expect(called(api, `DELETE ${P}/${IDEAS.id}/grants/${RUI.user_id}`)).toHaveLength(1);
    });
  });

  it("changes a role, and Undo gives back the one they had", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("reshare");
    await undo();
    await waitFor(() => {
      expect(called(api, `PUT ${P}/${IDEAS.id}/grants/${RUI.user_id}`)).toHaveLength(2);
    });
    expect(called(api, `PUT ${P}/${IDEAS.id}/grants/${RUI.user_id}`)[1]?.body).toEqual({
      role: "reader",
    });
  });

  it("stops sharing, and Undo shares again with the role they had", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("unshare");
    await undo();
    await waitFor(() => {
      expect(called(api, `PUT ${P}/${IDEAS.id}/grants/${RUI.user_id}`)).toHaveLength(1);
    });
    expect(called(api, `PUT ${P}/${IDEAS.id}/grants/${RUI.user_id}`)[0]?.body).toEqual({
      role: "reader",
    });
  });

  it("renames a page", async () => {
    const api = mockApi(routes());
    renderWithProviders(<Harness />);
    press("rename");
    await waitFor(() => {
      expect(called(api, `PATCH ${P}/${IDEAS.id}`)).toHaveLength(1);
    });
    expect(called(api, `PATCH ${P}/${IDEAS.id}`)[0]?.body).toMatchObject({ title: "Plan" });
  });

  it("says what failed, and runs it again on Retry", async () => {
    let failing = true;
    const api = mockApi(
      routes({
        [`POST ${P}/${IDEAS.id}/trash`]: () => {
          if (failing) {
            failing = false;
            return { status: 500, body: { status: 500, title: "Server error" } };
          }
          return { body: IDEAS };
        },
      }),
    );
    renderWithProviders(<Harness />);
    press("trash");
    fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(called(api, `POST ${P}/${IDEAS.id}/trash`)).toHaveLength(2);
    });
  });
});
