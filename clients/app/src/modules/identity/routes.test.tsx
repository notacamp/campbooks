import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { readTabValue, writeTabValue } from "~/lib/storage/tab";

import { NEW_ACCOUNT, REQUEST } from "../../../test/demo/identity";
import { mockApi, problem, renderWithProviders } from "../../../test/render";

import { NewAccountScreen } from "./components/NewAccount/NewAccount";
import { SignInScreen } from "./components/SignIn/SignInScreen";
import { buildIdentityRoutes } from "./index";

const SESSION = "GET /bff/session";
const REQUEST_INFO = `GET /auth/requests/${REQUEST}`;
const NEW = "GET /auth/new-account";
const CREATE = "POST /auth/new-account";

/** Renders `screen` as the page at `path`, in a router of its own. */
function renderAt(path: string, page: ReactNode) {
  const root = createRootRoute({ component: () => page });
  const router = createRouter({
    routeTree: root,
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  renderWithProviders(<RouterProvider router={router} />);
  return router;
}

describe("buildIdentityRoutes", () => {
  afterEach(() => {
    writeTabValue("signIn.returnTo", "");
  });

  it("mounts /sign-in and /sign-in/new with their query", async () => {
    mockApi({ [REQUEST_INFO]: { body: { client_id: "campbooks-web", providers: ["google"] } } });
    const root = createRootRoute({ component: Outlet });
    const routes = buildIdentityRoutes({ public: root, workspace: root });
    const router = createRouter({
      routeTree: root.addChildren([...routes.public]),
      history: createMemoryHistory({ initialEntries: [`/sign-in?request=${REQUEST}`] }),
    });
    renderWithProviders(<RouterProvider router={router} />);

    expect(await screen.findByRole("link", { name: /Continue with Google/u })).toBeVisible();
    await router.navigate({ to: "/sign-in/new", search: { request: REQUEST } });
    expect(router.state.location.pathname).toBe("/sign-in/new");
  });
});

describe("SignInScreen", () => {
  afterEach(() => {
    writeTabValue("signIn.returnTo", "");
  });

  it("starts a sign-in through the BFF, remembering where to come back to", async () => {
    mockApi({ [SESSION]: { body: { authenticated: false } } });
    const leave = vi.fn();
    renderAt("/sign-in", <SignInScreen returnTo="/w/1?tab=a" leave={leave} />);

    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/bff/login?return_to=%2Fw%2F1%3Ftab%3Da");
    });
    expect(screen.getByRole("heading", { name: "Opening sign-in…" })).toBeVisible();
    expect(readTabValue("signIn.returnTo")).toBe("/w/1?tab=a");
  });

  it("never comes back to a page off this site", async () => {
    mockApi({ [SESSION]: { body: { authenticated: false } } });
    const leave = vi.fn();
    renderAt("/sign-in", <SignInScreen returnTo="https://evil.example.com" leave={leave} />);

    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/bff/login?return_to=%2F");
    });
  });

  it("sends a browser already signed in back where it was going", async () => {
    mockApi({
      [SESSION]: { body: { authenticated: true, user: { sub: "ana", name: "Ana" } } },
    });
    const leave = vi.fn();
    const router = renderAt("/sign-in", <SignInScreen returnTo="/w/1" leave={leave} />);

    await waitFor(() => {
      expect(router.state.location.pathname).toBe("/w/1");
    });
    expect(leave).not.toHaveBeenCalled();
  });

  it("explains a callback error with no request, and starts again from the remembered page", async () => {
    writeTabValue("signIn.returnTo", "/w/2");
    mockApi({ [SESSION]: { body: { authenticated: false } } });
    const leave = vi.fn();
    renderAt("/sign-in", <SignInScreen error="invalid_state" leave={leave} />);

    expect(
      await screen.findByRole("heading", { name: "That sign-in can’t be finished" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Start again" }));
    expect(leave).toHaveBeenCalledWith("/bff/login?return_to=%2Fw%2F2");
  });

  it("offers to start again when the session can't be read", async () => {
    mockApi({ [SESSION]: { status: 503, body: { status: 503 } } });
    renderAt("/sign-in", <SignInScreen leave={vi.fn()} />);

    expect(
      await screen.findByRole("heading", { name: "Couldn’t load the ways to sign in" }),
    ).toBeVisible();
  });

  it("shows the ways in for a waiting request, with the callback's error", async () => {
    writeTabValue("signIn.returnTo", "/invitations/accept?t=1");
    mockApi({ [REQUEST_INFO]: { body: { client_id: "campbooks-web", providers: ["google"] } } });
    renderAt(
      "/sign-in",
      <SignInScreen request={REQUEST} error="provider_unavailable" leave={vi.fn()} />,
    );

    expect(
      await screen.findByRole("heading", { name: "Sign in to join the workspace" }),
    ).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("The sign-in provider didn’t answer.");
  });

  it("explains an invite-only instance from the callback", async () => {
    mockApi({ [REQUEST_INFO]: { body: { client_id: "campbooks-web", providers: ["google"] } } });
    renderAt(
      "/sign-in",
      <SignInScreen request={REQUEST} error="signup_invite_only" leave={vi.fn()} />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This Campbooks is by invitation only. Nobody has invited this address yet",
    );
  });

  it("says when the request expired, keeping the callback's error", async () => {
    mockApi({ [REQUEST_INFO]: { status: 404 } });
    const leave = vi.fn();
    renderAt("/sign-in", <SignInScreen request={REQUEST} error="login_failed" leave={leave} />);

    expect(
      await screen.findByRole("heading", { name: "This sign-in page has expired" }),
    ).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("couldn’t confirm who you are");
    fireEvent.click(screen.getByRole("button", { name: "Start again" }));
    expect(leave).toHaveBeenCalledWith("/bff/login?return_to=%2F");
  });

  it("offers to retry when the request can't be loaded", async () => {
    let calls = 0;
    mockApi({
      [REQUEST_INFO]: () => {
        calls += 1;
        return calls === 1
          ? { status: 503, body: { status: 503 } }
          : { body: { client_id: "campbooks-web", providers: ["google"] } };
      },
    });
    renderAt("/sign-in", <SignInScreen request={REQUEST} leave={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("link", { name: /Continue with Google/u })).toBeVisible();
  });
});

describe("NewAccountScreen", () => {
  afterEach(() => {
    writeTabValue("signIn.returnTo", "");
  });

  it("shows who the account is for and what happens next, and creates it with Enter", async () => {
    const api = mockApi({
      [NEW]: { body: NEW_ACCOUNT },
      [CREATE]: (request) => {
        expect(request.headers.get("x-campbooks-csrf")).toBe("1");
        return { status: 201, body: { redirect: "/bff/callback?code=c&state=s" } };
      },
    });
    const leave = vi.fn();
    renderWithProviders(<NewAccountScreen request={REQUEST} leave={leave} />);

    expect(
      await screen.findByRole("heading", { level: 1, name: "Create your Campbooks account" }),
    ).toBeVisible();
    expect(screen.getByText("No Campbooks account uses ana@example.com yet.")).toBeVisible();
    expect(screen.getByRole("list", { name: "Your new account" })).toHaveTextContent(
      "Ana DuarteGoogleana@example.com",
    );
    expect(screen.getAllByRole("listitem")).toHaveLength(4);
    const create = screen.getByRole("button", { name: /Create account/u });
    await waitFor(() => {
      expect(create).toHaveFocus();
    });
    expect(screen.getByRole("link", { name: /Sign in another way/u })).toHaveAttribute(
      "href",
      `/sign-in?request=${REQUEST}`,
    );

    fireEvent.submit(create);
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/bff/callback?code=c&state=s");
    });
    expect(api.calls.filter((call) => call.route === CREATE)).toHaveLength(1);
  });

  it("goes back to the ways in with Escape", async () => {
    mockApi({ [NEW]: { body: { provider: "google", email: null, display_name: null } } });
    const leave = vi.fn();
    renderWithProviders(<NewAccountScreen request={REQUEST} leave={leave} />);

    expect(
      await screen.findByText("No Campbooks account uses this Google account yet."),
    ).toBeVisible();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(leave).toHaveBeenCalledWith(`/sign-in?request=${REQUEST}`);
  });

  it("says the sign-up isn't waiting when nothing is pending for this browser", async () => {
    mockApi({ [NEW]: { status: 404 } });
    const leave = vi.fn();
    renderWithProviders(<NewAccountScreen leave={leave} />);

    expect(
      await screen.findByRole("heading", { name: "This sign-up isn’t waiting any more" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    expect(leave).toHaveBeenCalledWith("/bff/login?return_to=%2F");
  });

  it("says the sign-up expired when the server says so on Create", async () => {
    mockApi({ [NEW]: { body: NEW_ACCOUNT }, [CREATE]: problem(409, "sign_up_expired") });
    renderWithProviders(<NewAccountScreen request={REQUEST} leave={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: /Create account/u }));
    expect(
      await screen.findByRole("heading", { name: "This sign-up isn’t waiting any more" }),
    ).toBeVisible();
  });

  it("explains an identity another account took meanwhile", async () => {
    mockApi({
      [NEW]: { body: NEW_ACCOUNT },
      [CREATE]: problem(409, "way_in_belongs_to_another_account"),
    });
    renderWithProviders(<NewAccountScreen request={REQUEST} leave={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: /Create account/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This account already signs in to another Campbooks account.",
    );
  });

  it("explains an invite-only instance, with a way to sign in with another account", async () => {
    mockApi({ [NEW]: { body: NEW_ACCOUNT }, [CREATE]: problem(403, "signup_invite_only") });
    const leave = vi.fn();
    renderWithProviders(<NewAccountScreen request={REQUEST} leave={leave} />);

    fireEvent.click(await screen.findByRole("button", { name: /Create account/u }));
    expect(
      await screen.findByRole("heading", { name: "This Campbooks is by invitation only" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Sign in another way" }));
    expect(leave).toHaveBeenCalledWith("/bff/login?return_to=%2F");
  });

  it("says nothing was created when Create fails otherwise", async () => {
    mockApi({ [NEW]: { body: NEW_ACCOUNT }, [CREATE]: problem(403, "csrf_check_failed") });
    renderWithProviders(<NewAccountScreen request={REQUEST} leave={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: /Create account/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t create the account.");
  });

  it("offers to retry when the new account can't be loaded", async () => {
    let calls = 0;
    mockApi({
      [NEW]: () => {
        calls += 1;
        return calls === 1 ? { status: 503, body: { status: 503 } } : { body: NEW_ACCOUNT };
      },
    });
    renderWithProviders(<NewAccountScreen leave={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByText("No Campbooks account uses ana@example.com yet.")).toBeVisible();
  });
});
