import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { writeTabValue } from "~/lib/storage/tab";

import { REQUEST } from "../../../../../test/demo/identity";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";
import {
  ASSERTION_JSON,
  REQUEST_OPTIONS,
  domError,
  stubWebAuthn,
} from "../../../../../test/webauthn";

import { SignInChoices } from "./SignIn";

const OPTIONS = "POST /auth/passkey/sign-in/options";
const SIGN_IN = "POST /auth/passkey/sign-in";

describe("SignInChoices", () => {
  afterEach(() => {
    writeTabValue("signIn.returnTo", "");
  });

  it("offers Google with its key, and explains the one consent before leaving", () => {
    const leave = vi.fn();
    renderWithProviders(
      <SignInChoices request={REQUEST} providers={["google"]} returnTo="/" leave={leave} />,
    );

    expect(screen.getByRole("heading", { level: 1, name: "Sign in to Campbooks" })).toBeVisible();
    const google = screen.getByRole("link", { name: /Continue with Google/u });
    expect(google).toHaveAttribute("href", `/auth/start/google?request=${REQUEST}`);
    expect(google).toHaveAttribute("aria-keyshortcuts", "1");
    expect(screen.getByText(/also ask to let Campbooks use your Gmail/u)).toBeVisible();
    expect(screen.getByText("What Campbooks reads and keeps")).toBeVisible();
    expect(screen.getByText(/hasn’t verified this app/u)).toBeVisible();

    fireEvent.keyDown(document.body, { key: "1" });
    expect(leave).toHaveBeenCalledWith(`/auth/start/google?request=${REQUEST}`);
  });

  it("numbers every way in the server lists, with P for a passkey", async () => {
    const browser = stubWebAuthn();
    const api = mockApi({
      [OPTIONS]: { body: REQUEST_OPTIONS },
      [SIGN_IN]: { body: { redirect: "/bff/callback?code=c1" } },
    });
    const leave = vi.fn();
    renderWithProviders(
      <SignInChoices
        request={REQUEST}
        providers={["google", "microsoft", "zoho", "passkey"]}
        returnTo="/"
        leave={leave}
      />,
    );
    const ways = within(screen.getByRole("group", { name: "Ways to sign in" }));

    expect(ways.getAllByRole("link").map((link) => link.getAttribute("aria-keyshortcuts"))).toEqual(
      ["1", "2", "3"],
    );
    expect(ways.getByRole("button", { name: /Use a passkey/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "P",
    );
    fireEvent.keyDown(document.body, { key: "3" });
    fireEvent.keyDown(document.body, { key: "p" });
    await waitFor(() => {
      expect(leave).toHaveBeenCalledTimes(2);
    });
    expect(leave.mock.calls).toEqual([
      [`/auth/start/zoho?request=${REQUEST}`],
      ["/bff/callback?code=c1"],
    ]);
    expect(api.calls.map((call) => call.route)).toEqual([OPTIONS, SIGN_IN]);
    expect(api.calls[0]?.body).toEqual({ request: REQUEST });
    expect(api.calls[1]?.body).toEqual(ASSERTION_JSON);
    expect(browser.get.mock.calls[0]?.[0]).not.toHaveProperty("mediation");
  });

  it("leaves out the Gmail note when Google isn't offered", () => {
    renderWithProviders(<SignInChoices request={REQUEST} providers={["zoho"]} returnTo="/" />);

    expect(screen.getByRole("link", { name: /Continue with Zoho/u })).toBeVisible();
    expect(screen.queryByText(/Gmail/u)).not.toBeInTheDocument();
  });

  it("says when no way in is set up", () => {
    renderWithProviders(<SignInChoices request={REQUEST} providers={["facebook"]} returnTo="/" />);

    expect(screen.getByRole("heading", { name: "Signing in isn’t set up yet" })).toBeVisible();
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("explains a callback error above the ways in", () => {
    renderWithProviders(
      <SignInChoices
        request={REQUEST}
        providers={["google"]}
        error="cancelled_at_provider"
        returnTo="/"
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "You cancelled on the sign-in screen. Nothing was created.",
    );
    expect(screen.getByRole("link", { name: /Continue with Google/u })).toBeVisible();
  });

  it("uses the invitation copy on the way to an invitation", () => {
    renderWithProviders(
      <SignInChoices
        request={REQUEST}
        providers={["google"]}
        returnTo="/invitations/accept?token=t"
      />,
    );

    expect(
      screen.getByRole("heading", { level: 1, name: "Sign in to join the workspace" }),
    ).toBeVisible();
  });

  it("is translated", () => {
    renderWithProviders(
      <SignInChoices request={REQUEST} providers={["google"]} returnTo="/" />,
      "pt-PT",
    );

    expect(screen.getByRole("link", { name: /Continuar com Google/u })).toBeVisible();
  });

  it("hides passkeys in a browser that has none", () => {
    renderWithProviders(
      <SignInChoices request={REQUEST} providers={["google", "passkey"]} returnTo="/" />,
    );
    expect(screen.queryByRole("button", { name: /passkey/u })).toBeNull();
  });

  it("offers saved passkeys in a field's autofill, and signs in with the one picked", async () => {
    const browser = stubWebAuthn({ autofill: true });
    const api = mockApi({
      [OPTIONS]: { body: REQUEST_OPTIONS },
      [SIGN_IN]: { body: { redirect: "/bff/callback?code=c2" } },
    });
    const leave = vi.fn();
    renderWithProviders(
      <SignInChoices
        request={REQUEST}
        providers={["passkey"]}
        passkeySignUp
        returnTo="/"
        leave={leave}
      />,
    );

    const field = await screen.findByRole("textbox", { name: "Saved passkeys" });
    expect(field).toHaveAttribute("autocomplete", "username webauthn");
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/bff/callback?code=c2");
    });
    expect(browser.get.mock.calls[0]?.[0]).toMatchObject({ mediation: "conditional" });
    expect(api.calls[1]?.route).toBe(SIGN_IN);
    expect(screen.getByRole("link", { name: /Create an account with a passkey/u })).toHaveAttribute(
      "href",
      `/sign-in/passkey?request=${REQUEST}`,
    );
  });

  it("says a passkey's account is suspended once the passkey verified", async () => {
    stubWebAuthn();
    mockApi({
      [OPTIONS]: { body: REQUEST_OPTIONS },
      [SIGN_IN]: problem(403, "account_suspended"),
    });
    renderWithProviders(
      <SignInChoices request={REQUEST} providers={["google", "passkey"]} returnTo="/" />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Use a passkey/u }));
    expect(await screen.findByText("This account is suspended.")).toBeVisible();
  });

  it("explains a passkey that wasn't accepted, and one that was cancelled", async () => {
    const browser = stubWebAuthn();
    mockApi({
      [OPTIONS]: { body: REQUEST_OPTIONS },
      [SIGN_IN]: problem(403, "passkey_not_accepted"),
    });
    renderWithProviders(
      <SignInChoices request={REQUEST} providers={["google", "passkey"]} returnTo="/" />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Use a passkey/u }));
    expect(await screen.findByText("That passkey wasn’t accepted.")).toBeVisible();

    browser.get.mockRejectedValueOnce(domError("NotAllowedError"));
    fireEvent.click(screen.getByRole("button", { name: /Use a passkey/u }));
    expect(await screen.findByText("No passkey was used.")).toBeVisible();
    expect(screen.getByRole("button", { name: /Use a passkey/u })).toBeEnabled();
  });

  it("says when the ceremony took too long or was tried too often", async () => {
    stubWebAuthn();
    mockApi({
      [OPTIONS]: { body: REQUEST_OPTIONS },
      [SIGN_IN]: problem(409, "passkey_expired"),
    });
    renderWithProviders(<SignInChoices request={REQUEST} providers={["passkey"]} returnTo="/" />);
    fireEvent.keyDown(document.body, { key: "p" });
    expect(await screen.findByText("That took too long.")).toBeVisible();

    mockApi({ [OPTIONS]: problem(429, "rate_limited") });
    fireEvent.keyDown(document.body, { key: "p" });
    expect(await screen.findByText("Too many tries.")).toBeVisible();

    mockApi({ [OPTIONS]: problem(503, "unavailable") });
    fireEvent.keyDown(document.body, { key: "p" });
    expect(await screen.findByText("Couldn’t sign in with a passkey.")).toBeVisible();
  });
});
