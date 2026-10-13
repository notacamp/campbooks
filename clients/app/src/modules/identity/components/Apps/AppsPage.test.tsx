import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CONNECTED_APPS, DEMO_SECRET, WORKSPACE_CLIENTS } from "../../../../../test/demo/identity";
import { DUARTE, HOUSEHOLD } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { AppsPage } from "./AppsPage";

const WORKSPACES = "GET /bff/api/v1/workspaces";
const CONSENTS = "GET /bff/api/v1/identity/consents";
const REVOKE = (id: string) => `DELETE /bff/api/v1/identity/consents/${id}`;
const CLIENTS = (ws: string) => `GET /bff/api/v1/w/${ws}/identity/clients`;
const REGISTER = `POST /bff/api/v1/w/${DUARTE.id}/identity/clients`;
const ROTATE = (id: string) => `POST /bff/api/v1/w/${DUARTE.id}/identity/clients/${id}/secret`;
const HERE = `/w/${DUARTE.id}/settings/apps`;

function renderPage(workspaceId: string = DUARTE.id) {
  const leave = vi.fn();
  renderWithProviders(<AppsPage workspaceId={workspaceId} returnTo={HERE} leave={leave} />);
  return leave;
}

const base = {
  [WORKSPACES]: { body: { workspaces: [DUARTE, HOUSEHOLD] } },
  [CONSENTS]: { body: { items: CONNECTED_APPS } },
  [CLIENTS(DUARTE.id)]: { body: { items: WORKSPACE_CLIENTS } },
};

describe("AppsPage", () => {
  it("lists the apps you let in, where, and what they can do", async () => {
    mockApi(base);
    renderPage();

    const list = await screen.findByRole("list", { name: "Apps you let in" });
    const [agent, desktop] = within(list).getAllByRole("listitem");
    if (agent === undefined || desktop === undefined) {
      throw new Error("two rows expected");
    }
    expect(within(agent).getByText("Desk Agent")).toBeVisible();
    expect(within(agent).getByText("Unverified")).toBeVisible();
    expect(within(agent).getByText(/In Duarte & Pessoa · Allowed/u)).toBeVisible();
    expect(
      within(agent).getByText(/Read your conversations and messages · Stay connected/u),
    ).toBeVisible();
    expect(within(desktop).getByText(/All your workspaces/u)).toBeVisible();
  });

  it("revokes an app only after confirming in place", async () => {
    const api = mockApi({ ...base, [REVOKE(CONNECTED_APPS[0]?.id ?? "")]: { status: 204 } });
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Revoke Desk Agent" }));
    expect(api.calls.some((call) => call.route.startsWith("DELETE"))).toBe(false);
    expect(
      screen.getByText("Desk Agent stops working at once, until you allow it again."),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Revoke" }));
    await waitFor(() => {
      expect(screen.queryByText("Desk Agent")).toBeNull();
    });
  });

  it("creates a server-to-server app and shows its secret once, with Copy", async () => {
    const api = mockApi({
      ...base,
      [REGISTER]: {
        status: 201,
        body: {
          client: { ...WORKSPACE_CLIENTS[0], client_id: "cb_new", name: "Weekly report" },
          client_secret: DEMO_SECRET,
        },
      },
    });
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: /New app/u }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Weekly report" },
    });
    fireEvent.click(screen.getByRole("radio", { name: "Read and change" }));
    fireEvent.click(screen.getByRole("button", { name: "Create app" }));

    expect(await screen.findByText("Copy the secret now")).toBeVisible();
    expect(screen.getByDisplayValue(DEMO_SECRET)).toBeVisible();
    expect(screen.getByDisplayValue("cb_new")).toBeVisible();
    const sent = api.calls.find((call) => call.route === REGISTER)?.body as {
      name: string;
      type: string;
      scopes: string[];
    };
    expect(sent.name).toBe("Weekly report");
    expect(sent.type).toBe("confidential");
    expect(sent.scopes).toContain("conversations:write");
    expect(sent.scopes.some((scope) => scope.startsWith("identity:"))).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByDisplayValue(DEMO_SECRET)).toBeNull();
  });

  it("checks a sign-in app's return address before sending it", async () => {
    const api = mockApi(base);
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: /New app/u }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Dashboard" },
    });
    fireEvent.click(screen.getByRole("radio", { name: "Sign-in app" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Return address" }), {
      target: { value: "http://dashboard.example.com/cb" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create app" }));
    expect(
      await screen.findByText("Use an https address, or http://127.0.0.1 with a port."),
    ).toBeVisible();
    expect(api.calls.some((call) => call.route === REGISTER)).toBe(false);
  });

  it("rotates a secret after confirming, and asks for a fresh sign-in when needed", async () => {
    const client = WORKSPACE_CLIENTS[0];
    if (client === undefined) {
      throw new Error("demo client");
    }
    mockApi({ ...base, [ROTATE(client.client_id)]: problem(403, "step_up_required") });
    const leave = renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: "Rotate the secret of Nightly export" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Rotate secret" }));
    const alert = await screen.findByRole("alert");
    fireEvent.click(within(alert).getByRole("button", { name: "Sign in again" }));
    expect(leave).toHaveBeenCalledWith(
      `/bff/login?return_to=${encodeURIComponent(HERE)}&max_age=600`,
    );
  });

  it("tells a member that only owners and admins manage the workspace's apps", async () => {
    mockApi(base);
    renderPage(HOUSEHOLD.id);

    expect(
      await screen.findByText("Only owners and admins can manage this workspace’s apps."),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: /New app/u })).toBeNull();
  });
});
