import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { REQUEST } from "../../../../../test/demo/identity";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";
import {
  ATTESTATION_JSON,
  CREATION_OPTIONS,
  domError,
  stubWebAuthn,
} from "../../../../../test/webauthn";

import { PasskeySignUpScreen } from "./PasskeySignUp";

const OPTIONS = "POST /auth/passkey/sign-up/options";
const SIGN_UP = "POST /auth/passkey/sign-up";

function renderScreen() {
  const leave = vi.fn();
  renderWithProviders(<PasskeySignUpScreen request={REQUEST} leave={leave} />);
  return leave;
}

describe("PasskeySignUpScreen", () => {
  it("asks one question, then makes the passkey and signs in with Enter", async () => {
    const browser = stubWebAuthn();
    const api = mockApi({
      [OPTIONS]: { body: CREATION_OPTIONS },
      [SIGN_UP]: { status: 201, body: { redirect: "/bff/callback?code=c3" } },
    });
    const leave = renderScreen();

    expect(
      screen.getByRole("heading", { level: 1, name: "Create an account with a passkey" }),
    ).toBeVisible();
    const name = screen.getByRole("textbox", { name: "What should we call you?" });
    expect(name).toHaveFocus();
    expect(screen.getByText("Your device asks to save a passkey for Campbooks.")).toBeVisible();
    fireEvent.change(name, { target: { value: "  Ana Duarte " } });
    fireEvent.submit(name);

    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/bff/callback?code=c3");
    });
    expect(api.calls.map((call) => call.route)).toEqual([OPTIONS, SIGN_UP]);
    expect(api.calls[0]?.body).toEqual({ display_name: "Ana Duarte", request: REQUEST });
    expect(api.calls[1]?.body).toEqual(ATTESTATION_JSON);
    expect(browser.create).toHaveBeenCalledTimes(1);
  });

  it("needs a name, of 64 characters at most", () => {
    stubWebAuthn();
    const api = mockApi({});
    renderScreen();
    const name = screen.getByRole("textbox", { name: "What should we call you?" });
    fireEvent.submit(name);
    expect(screen.getByText("Tell us what to call you.")).toBeVisible();
    fireEvent.change(name, { target: { value: "x".repeat(65) } });
    fireEvent.submit(name);
    expect(screen.getByText("Use 64 characters or fewer.")).toBeVisible();
    expect(api.calls).toEqual([]);
  });

  it("explains a cancelled passkey and lets it be tried again", async () => {
    const browser = stubWebAuthn();
    mockApi({ [OPTIONS]: { body: CREATION_OPTIONS } });
    browser.create.mockRejectedValueOnce(domError("NotAllowedError"));
    renderScreen();
    const name = screen.getByRole("textbox", { name: "What should we call you?" });
    fireEvent.change(name, { target: { value: "Ana" } });
    fireEvent.submit(name);
    expect(await screen.findByText("No passkey was made.")).toBeVisible();
    expect(screen.getByRole("button", { name: /Create with a passkey/u })).toBeEnabled();
  });

  it("says an invite-only instance takes no passkey sign-ups", async () => {
    stubWebAuthn();
    mockApi({ [OPTIONS]: problem(403, "signup_invite_only") });
    const leave = renderScreen();
    const name = screen.getByRole("textbox", { name: "What should we call you?" });
    fireEvent.change(name, { target: { value: "Ana" } });
    fireEvent.submit(name);
    expect(
      await screen.findByRole("heading", { name: "This Campbooks takes invited people only" }),
    ).toBeVisible();
    expect(screen.getByRole("link", { name: "Sign in another way" })).toHaveAttribute(
      "href",
      `/sign-in?request=${REQUEST}`,
    );
    expect(leave).not.toHaveBeenCalled();
  });

  it("explains server refusals in place", async () => {
    stubWebAuthn();
    const name = () => screen.getByRole("textbox", { name: "What should we call you?" });
    mockApi({ [OPTIONS]: { body: CREATION_OPTIONS }, [SIGN_UP]: problem(409, "passkey_expired") });
    renderScreen();
    fireEvent.change(name(), { target: { value: "Ana" } });
    fireEvent.submit(name());
    expect(await screen.findByText("That took too long.")).toBeVisible();

    mockApi({
      [OPTIONS]: { body: CREATION_OPTIONS },
      [SIGN_UP]: problem(403, "passkey_not_accepted"),
    });
    fireEvent.submit(name());
    expect(await screen.findByText("That passkey wasn’t accepted.")).toBeVisible();

    mockApi({ [OPTIONS]: problem(400, "display_name_required") });
    fireEvent.submit(name());
    expect(await screen.findByText("Tell us what to call you.")).toBeVisible();

    mockApi({ [OPTIONS]: problem(500, "internal_error") });
    fireEvent.submit(name());
    expect(await screen.findByText("Couldn’t create the account.")).toBeVisible();
  });

  it("says when this device already holds a passkey for it", async () => {
    const browser = stubWebAuthn();
    mockApi({ [OPTIONS]: { body: CREATION_OPTIONS } });
    browser.create.mockRejectedValueOnce(domError("InvalidStateError"));
    renderScreen();
    const name = screen.getByRole("textbox", { name: "What should we call you?" });
    fireEvent.change(name, { target: { value: "Ana" } });
    fireEvent.submit(name);
    expect(await screen.findByText("This device already has a passkey for that.")).toBeVisible();
  });

  it("goes back to the ways in with Escape", () => {
    stubWebAuthn();
    const leave = renderScreen();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(leave).toHaveBeenCalledWith(`/sign-in?request=${REQUEST}`);
  });

  it("says when the browser has no passkeys", () => {
    renderScreen();
    expect(screen.getByRole("heading", { name: "This browser can’t use passkeys" })).toBeVisible();
  });
});
