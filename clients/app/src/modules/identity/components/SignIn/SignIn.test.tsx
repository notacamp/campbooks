import { fireEvent, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { writeTabValue } from "~/lib/storage/tab";

import { REQUEST } from "../../../../../test/demo/identity";
import { renderWithProviders } from "../../../../../test/render";

import { SignInChoices } from "./SignIn";

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

  it("numbers every way in the server lists, with P for a passkey", () => {
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
      ["1", "2", "3", "P"],
    );
    fireEvent.keyDown(document.body, { key: "3" });
    fireEvent.keyDown(document.body, { key: "p" });
    expect(leave.mock.calls).toEqual([
      [`/auth/start/zoho?request=${REQUEST}`],
      [`/auth/start/passkey?request=${REQUEST}`],
    ]);
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
});
