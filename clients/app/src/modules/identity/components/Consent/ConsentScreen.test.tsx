import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  AGENT_CONSENT,
  NATIVE_CONSENT,
  REQUEST,
  TEAM_APP_CONSENT,
} from "../../../../../test/demo/identity";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ConsentScreen } from "./ConsentScreen";

const SHOWN = `GET /auth/consent/${REQUEST}`;
const ANSWER = `POST /auth/consent/${REQUEST}`;

function renderScreen() {
  const leave = vi.fn();
  renderWithProviders(<ConsentScreen request={REQUEST} leave={leave} />);
  return leave;
}

describe("ConsentScreen", () => {
  it("names an unverified agent, warns about this device, and asks for a workspace", async () => {
    const api = mockApi({
      [SHOWN]: { body: AGENT_CONSENT },
      [ANSWER]: { body: { redirect: "http://127.0.0.1:33418/callback?code=c1" } },
    });
    const leave = renderScreen();

    expect(
      await screen.findByRole("heading", {
        name: "Desk Agent wants to use your Campbooks account",
      }),
    ).toBeVisible();
    expect(screen.getByText("You’re signed in as ana@example.com.")).toBeVisible();
    expect(screen.getByText("Unverified")).toBeVisible();
    expect(screen.getByText("Only if you just started this")).toBeVisible();
    expect(screen.getByText("Read your conversations and messages")).toBeVisible();
    expect(screen.getByText("Next, you’ll go back to an app on this device.")).toBeVisible();
    const allow = screen.getByRole("button", { name: /Allow/u });
    expect(allow).toBeDisabled();

    fireEvent.click(screen.getByRole("radio", { name: "Household" }));
    expect(allow).toBeEnabled();
    fireEvent.click(allow);
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("http://127.0.0.1:33418/callback?code=c1");
    });
    expect(api.calls.find((call) => call.route === ANSWER)?.body).toEqual({
      decision: "allow",
      workspace_id: "0192f1a0-0000-7000-8000-000000000002",
    });
  });

  it("lets the desktop app in for every workspace, with no picker", async () => {
    const api = mockApi({
      [SHOWN]: { body: NATIVE_CONSENT },
      [ANSWER]: { body: { redirect: "http://127.0.0.1:5555/callback?code=c2" } },
    });
    const leave = renderScreen();

    expect(
      await screen.findByText("It will work in all your workspaces, like Campbooks on the web."),
    ).toBeVisible();
    expect(screen.queryByRole("radio")).toBeNull();
    expect(screen.getByText("Verified")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Allow/u }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalled();
    });
    expect(api.calls.find((call) => call.route === ANSWER)?.body).toEqual({
      decision: "allow",
    });
  });

  it("picks the only workspace of a workspace's own app and says it will be remembered", async () => {
    mockApi({ [SHOWN]: { body: TEAM_APP_CONSENT } });
    renderScreen();

    expect(await screen.findByRole("radio", { name: "Duarte & Pessoa" })).toBeChecked();
    expect(screen.getByText("Next, you’ll go to dashboard.example.com.")).toBeVisible();
    expect(screen.getByText(/You won’t be asked again for this app/u)).toBeVisible();
    expect(screen.queryByText("Only if you just started this")).toBeNull();
  });

  it("denies with Esc, sending the app its answer", async () => {
    const api = mockApi({
      [SHOWN]: { body: NATIVE_CONSENT },
      [ANSWER]: { body: { redirect: "http://127.0.0.1:5555/callback?error=access_denied" } },
    });
    const leave = renderScreen();
    await screen.findByRole("button", { name: /Deny/u });

    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("http://127.0.0.1:5555/callback?error=access_denied");
    });
    expect(api.calls.find((call) => call.route === ANSWER)?.body).toEqual({ decision: "deny" });
  });

  it("asks for a fresh sign-in when the last one is too old", async () => {
    mockApi({ [SHOWN]: { body: NATIVE_CONSENT }, [ANSWER]: problem(403, "step_up_required") });
    const leave = renderScreen();

    fireEvent.click(await screen.findByRole("button", { name: /Allow/u }));
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Sign in again first")).toBeVisible();
    fireEvent.click(within(alert).getByRole("button", { name: "Sign in again" }));
    expect(leave).toHaveBeenCalledWith(`/sign-in?request=${REQUEST}`);
  });

  it("explains a request that is no longer waiting", async () => {
    mockApi({ [SHOWN]: problem(404, "not_found") });
    renderScreen();

    expect(
      await screen.findByRole("heading", { name: "This request is no longer waiting" }),
    ).toBeVisible();
    expect(screen.getByRole("link", { name: "Go to Campbooks" })).toHaveAttribute("href", "/");
  });

  it("refuses to allow before anyone is in a workspace", async () => {
    mockApi({ [SHOWN]: { body: { ...AGENT_CONSENT, workspaces: [] } } });
    renderScreen();

    expect(await screen.findByText("Join a workspace first")).toBeVisible();
    expect(screen.getByRole("button", { name: /Allow/u })).toBeDisabled();
  });
});
