import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  AGENT_CONSENT,
  INSTANCE_APP_CONSENT,
  NATIVE_CONSENT,
  REQUEST,
  TEAM_APP_CONSENT,
} from "../../../../../test/demo/identity";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { ARM_DELAY_MS, ConsentScreen } from "./ConsentScreen";

/** Allow, once the screen has been in view long enough to work. */
async function armedAllow() {
  const allow = screen.getByRole("button", { name: "Allow" });
  await waitFor(
    () => {
      expect(allow).toBeEnabled();
    },
    { timeout: ARM_DELAY_MS * 3 },
  );
  return allow;
}

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
    const allow = screen.getByRole("button", { name: "Allow" });
    expect(allow).toBeDisabled();
    // Focus starts on Deny; nothing is picked for the person.
    expect(screen.getByRole("button", { name: /Deny/u })).toHaveFocus();
    expect(screen.getByRole("radio", { name: "Household" })).not.toBeChecked();

    fireEvent.click(screen.getByRole("radio", { name: "Household" }));
    fireEvent.click(await armedAllow());
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
    expect(screen.getAllByText("Campbooks").length).toBeGreaterThan(0);
    fireEvent.click(await armedAllow());
    await waitFor(() => {
      expect(leave).toHaveBeenCalled();
    });
    expect(api.calls.find((call) => call.route === ANSWER)?.body).toEqual({
      decision: "allow",
    });
  });

  it("names a workspace's own app by its workspace and picks nothing for the person", async () => {
    mockApi({ [SHOWN]: { body: TEAM_APP_CONSENT } });
    renderScreen();

    expect(await screen.findByRole("radio", { name: "Duarte & Pessoa" })).not.toBeChecked();
    expect(screen.getByText("Duarte & Pessoa’s app")).toBeVisible();
    expect(screen.getByText("Registered by an admin of Duarte & Pessoa.")).toBeVisible();
    expect(screen.getByText("Next, you’ll go to dashboard.example.com.")).toBeVisible();
    expect(screen.getByText(/You won’t be asked again for this app/u)).toBeVisible();
    expect(screen.queryByText("Only if you just started this")).toBeNull();
  });

  it("vouches for an app this server's admin added and asks for a workspace", async () => {
    const api = mockApi({
      [SHOWN]: { body: INSTANCE_APP_CONSENT },
      [ANSWER]: { body: { redirect: "https://applicamp.example.com/auth/callback?code=c3" } },
    });
    const leave = renderScreen();

    expect(
      await screen.findByRole("heading", {
        name: "Applicamp wants to use your Campbooks account",
      }),
    ).toBeVisible();
    expect(screen.getByText("Added by this server")).toBeVisible();
    expect(screen.getByText("Added to this Campbooks by the people who run it.")).toBeVisible();
    expect(screen.queryByText("Unverified")).toBeNull();
    expect(screen.queryByText("Anyone can register an app with any name")).toBeNull();
    expect(screen.getByText("See your calendars and events")).toBeVisible();
    expect(screen.getByText("Next, you’ll go to applicamp.example.com.")).toBeVisible();
    expect(screen.getByText(/You won’t be asked again for this app/u)).toBeVisible();
    expect(screen.getByRole("radio", { name: "Household" })).not.toBeChecked();

    fireEvent.click(screen.getByRole("radio", { name: "Household" }));
    fireEvent.click(await armedAllow());
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("https://applicamp.example.com/auth/callback?code=c3");
    });
    expect(api.calls.find((call) => call.route === ANSWER)?.body).toEqual({
      decision: "allow",
      workspace_id: "0192f1a0-0000-7000-8000-000000000002",
    });
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

    await screen.findByRole("button", { name: /Deny/u });
    fireEvent.click(await armedAllow());
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
    expect(screen.getByRole("button", { name: "Allow" })).toBeDisabled();
  });

  it("answers nothing to a key pressed at once, nor to Enter or ⌘↵ later", async () => {
    const api = mockApi({ [SHOWN]: { body: NATIVE_CONSENT } });
    renderScreen();
    const allow = await screen.findByRole("button", { name: "Allow" });

    fireEvent.click(allow);
    fireEvent.keyDown(document, { key: "Enter" });
    fireEvent.keyDown(document, { key: "Enter", metaKey: true });
    fireEvent.keyDown(document, { key: "Enter", ctrlKey: true });
    await armedAllow();
    fireEvent.keyDown(document, { key: "Enter", metaKey: true });
    fireEvent.keyDown(document, { key: "Enter", ctrlKey: true });
    expect(api.calls.some((call) => call.route === ANSWER)).toBe(false);
  });

  it("waits again after the page was hidden or left", async () => {
    const page = globalThis as unknown as Window;
    mockApi({ [SHOWN]: { body: NATIVE_CONSENT } });
    renderScreen();
    await screen.findByRole("button", { name: "Allow" });
    const allow = await armedAllow();

    fireEvent.blur(page);
    expect(allow).toBeDisabled();
    fireEvent.focus(page);
    expect(allow).toBeDisabled();
    await armedAllow();
  });
});
