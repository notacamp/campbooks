import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { W } from "../../../../test/demo/conversations";
import { HANDBOOK, plainDoc, viewOfDoc } from "../../../../test/demo/pages";
import { TEAM } from "../../../../test/demo/tasks";
import { ANA, DUARTE, RUI, SOFIA } from "../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../test/render";
import type { PageView } from "../types";

import { ShareMenu } from "./ShareMenu";
import { usePageActions } from "./use-page-actions";

const P = `${W}/pages/${HANDBOOK.id}`;

function Menu({ page }: { page: PageView }) {
  const actions = usePageActions(DUARTE.id);
  return <ShareMenu workspaceId={DUARTE.id} page={page} actions={actions} />;
}

const shared = (role: PageView["role"] = "manager") =>
  viewOfDoc({ ...HANDBOOK, created_by: ANA.user_id }, plainDoc("x"), {
    role,
    grants: [{ member: RUI.user_id, role: "reader" }],
  });

function api() {
  return mockApi({
    [`GET ${W}/members`]: { body: { members: TEAM } },
    [`PUT ${P}/grants/${RUI.user_id}`]: { body: { member: RUI.user_id, role: "writer" } },
    [`PUT ${P}/grants/${SOFIA.user_id}`]: { body: { member: SOFIA.user_id, role: "writer" } },
    [`DELETE ${P}/grants/${RUI.user_id}`]: { status: 204 },
    [`DELETE ${P}/grants/${SOFIA.user_id}`]: { status: 204 },
  });
}

const open = async () => {
  fireEvent.click(screen.getByRole("button", { name: "Share" }));
  await screen.findByText("People with access");
};
const calls = (mocked: ReturnType<typeof api>, route: string) =>
  mocked.calls.filter((call) => call.route === route);

describe("the share menu", () => {
  it("says where the page is and who has it", async () => {
    api();
    renderWithProviders(<Menu page={shared()} />);
    await open();
    expect(screen.getByText("Everyone in the workspace")).toBeTruthy();
    expect(await screen.findByText("Rui Matos")).toBeTruthy();
    expect(screen.getByText("Owner")).toBeTruthy();
    expect(
      screen.getByText("Mentioning someone in a page doesn’t share it with them."),
    ).toBeTruthy();
  });

  it("changes a role, and Undo gives back the old one", async () => {
    const mocked = api();
    renderWithProviders(<Menu page={shared()} />);
    await open();
    const select = await screen.findByRole("combobox", { name: "Access for Rui Matos" });
    fireEvent.change(select, { target: { value: "writer" } });
    await waitFor(() => {
      expect(calls(mocked, `PUT ${P}/grants/${RUI.user_id}`)).toHaveLength(1);
    });
    expect(calls(mocked, `PUT ${P}/grants/${RUI.user_id}`)[0]?.body).toEqual({ role: "writer" });
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(calls(mocked, `PUT ${P}/grants/${RUI.user_id}`)).toHaveLength(2);
    });
    expect(calls(mocked, `PUT ${P}/grants/${RUI.user_id}`)[1]?.body).toEqual({ role: "reader" });
  });

  it("removes a person, and Undo shares again", async () => {
    const mocked = api();
    renderWithProviders(<Menu page={shared()} />);
    await open();
    fireEvent.click(await screen.findByRole("button", { name: "Remove Rui Matos" }));
    await waitFor(() => {
      expect(calls(mocked, `DELETE ${P}/grants/${RUI.user_id}`)).toHaveLength(1);
    });
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(calls(mocked, `PUT ${P}/grants/${RUI.user_id}`)).toHaveLength(1);
    });
  });

  it("adds a person who is not yet on it, and Undo removes them", async () => {
    const mocked = api();
    renderWithProviders(<Menu page={shared()} />);
    await open();
    const person = await screen.findByRole("combobox", { name: "Person" });
    // Not the owner, not you, not someone who already has it.
    expect([...person.querySelectorAll("option")].map((option) => option.textContent)).toEqual([
      "Sofia Pessoa",
    ]);
    const buttons = screen.getAllByRole("button", { name: "Share" });
    fireEvent.click(buttons.at(-1) ?? document.body);
    await waitFor(() => {
      expect(calls(mocked, `PUT ${P}/grants/${SOFIA.user_id}`)).toHaveLength(1);
    });
    expect(calls(mocked, `PUT ${P}/grants/${SOFIA.user_id}`)[0]?.body).toEqual({ role: "writer" });
    fireEvent.click(await screen.findByRole("button", { name: /^Undo/u }));
    await waitFor(() => {
      expect(calls(mocked, `DELETE ${P}/grants/${SOFIA.user_id}`)).toHaveLength(1);
    });
  });

  it("only lets a manager change who has it", async () => {
    api();
    renderWithProviders(<Menu page={shared("writer")} />);
    await open();
    expect(await screen.findByText("Rui Matos")).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Person" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Remove Rui Matos" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Access for Rui Matos" })).toBeNull();
    expect(screen.getByText("Can read")).toBeTruthy();
  });
});
