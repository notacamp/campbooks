import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ALL_PROVIDERS,
  CONNECTION,
  GOOGLE_WAY_IN,
  SIGN_UP_PASSKEY,
  ZOHO_WAY_IN,
} from "../../../../../test/demo/identity";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { PasskeysSection } from "../Passkeys/PasskeysSection";

import { WaysInPage } from "./WaysInPage";

const WAYS = "GET /bff/api/v1/identity/ways-in";
const CONNECTIONS = `GET /bff/api/v1/w/${DUARTE.id}/connections`;
const LINK = "POST /bff/api/v1/identity/links";
const REMOVE = (id: string) => `DELETE /bff/api/v1/identity/ways-in/${id}`;
const HERE = `/w/${DUARTE.id}/settings/sign-in`;
const PASSKEYS = "GET /bff/api/v1/identity/passkeys";
const EVERYWHERE = "POST /bff/api/v1/identity/sign-out-everywhere";
const LOGOUT = "POST /bff/logout";

function renderPage(props: Partial<Parameters<typeof WaysInPage>[0]> = {}) {
  const leave = vi.fn();
  renderWithProviders(
    <WaysInPage workspaceId={DUARTE.id} returnTo={HERE} leave={leave} {...props} />,
  );
  return leave;
}

const listed = (items: unknown[], providers: readonly string[] = ALL_PROVIDERS) => ({
  [WAYS]: { body: { items, providers } },
  [CONNECTIONS]: { body: { connections: [] } },
  [PASSKEYS]: { body: { items: [] } },
});

/** The demo data's week: Zoho was added two days ago. */
function onOctober14() {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-10-14T12:00:00Z"));
}

describe("WaysInPage", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("lists every way in, newest last, with how and when it was added", async () => {
    mockApi(listed([GOOGLE_WAY_IN, ZOHO_WAY_IN]));
    renderPage();

    const list = await screen.findByRole("list", { name: "Your ways to sign in" });
    const [google, zoho, ...rest] = within(list).getAllByRole("listitem");
    expect(rest).toHaveLength(0);
    if (google === undefined || zoho === undefined) {
      throw new Error("two rows expected");
    }
    expect(within(google).getByText("Google")).toBeVisible();
    expect(within(google).getByText("Signed up with")).toBeVisible();
    expect(
      within(google).getByText(/ana@example.com · Added Oct 1, 2026 · Last used/u),
    ).toBeVisible();
    expect(within(zoho).getByText("Zoho")).toBeVisible();
    expect(screen.getByRole("button", { name: /Add a Microsoft account/u })).toBeVisible();
  });

  it("adds a way in through the front door, coming back here", async () => {
    const api = mockApi({
      ...listed([GOOGLE_WAY_IN]),
      [LINK]: { status: 201, body: { url: "/auth/start/zoho?link=t1", expires_at: "x" } },
    });
    const leave = renderPage();

    fireEvent.click(await screen.findByRole("button", { name: /Add a Zoho account/u }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/zoho?link=t1");
    });
    expect(api.calls.find((call) => call.route === LINK)?.body).toEqual({
      purpose: "link",
      provider: "zoho",
      return_to: HERE,
    });
  });

  it("asks for a fresh sign-in when the last one is too old", async () => {
    mockApi({ ...listed([GOOGLE_WAY_IN]), [LINK]: problem(403, "step_up_required") });
    const leave = renderPage();

    fireEvent.click(await screen.findByRole("button", { name: /Add a Google account/u }));
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Sign in again first")).toBeVisible();
    fireEvent.click(within(alert).getByRole("button", { name: "Sign in again" }));
    expect(leave).toHaveBeenCalledWith(
      `/bff/login?return_to=${encodeURIComponent(HERE)}&max_age=600`,
    );
  });

  it("removes a way in only after confirming in place", async () => {
    const api = mockApi({
      ...listed([GOOGLE_WAY_IN, ZOHO_WAY_IN]),
      [REMOVE(ZOHO_WAY_IN.id)]: { status: 204 },
    });
    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: "Remove Zoho (ana.duarte@example.net)" }),
    );
    expect(screen.getByText(/Any mailbox you connected with it stays connected\.$/u)).toBeVisible();
    expect(api.calls.map((call) => call.route)).not.toContain(REMOVE(ZOHO_WAY_IN.id));
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => {
      expect(screen.queryByText("Zoho")).toBeNull();
    });
  });

  it("explains why the last way in stays", async () => {
    mockApi({
      ...listed([GOOGLE_WAY_IN]),
      [REMOVE(GOOGLE_WAY_IN.id)]: problem(409, "last_way_in"),
    });
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: /^Remove Google/u }));
    expect(screen.getByText(/You need another way to sign in first/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByText("This is your only way to sign in.")).toBeVisible();
    expect(screen.getByText("Google")).toBeVisible();
  });

  it("offers to reconnect a mailbox of yours that stopped syncing", async () => {
    const api = mockApi({
      [WAYS]: { body: { items: [GOOGLE_WAY_IN], providers: ["google"] } },
      [CONNECTIONS]: { body: { connections: [{ ...CONNECTION, health: "needs_reauth" }] } },
      [LINK]: { status: 201, body: { url: "/auth/start/google?link=t2", expires_at: "x" } },
    });
    const leave = renderPage();

    expect(await screen.findByText(/stopped syncing/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Reconnect the mailbox ana@example.com" }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/auth/start/google?link=t2");
    });
    expect(api.calls.find((call) => call.route === LINK)?.body).toEqual({
      purpose: "reconnect",
      workspace_id: DUARTE.id,
      connection_id: CONNECTION.id,
      return_to: HERE,
    });
  });

  it("never offers to reconnect a colleague's mailbox at the same address", async () => {
    mockApi({
      [WAYS]: { body: { items: [GOOGLE_WAY_IN], providers: ["google"] } },
      [CONNECTIONS]: {
        body: {
          connections: [{ ...CONNECTION, health: "needs_reauth", granted_by_you: false }],
        },
      },
    });
    renderPage();

    expect(await screen.findByText("Google")).toBeVisible();
    expect(screen.queryByRole("button", { name: /^Reconnect/u })).toBeNull();
  });

  it("says what the front door came back with", async () => {
    mockApi(listed([GOOGLE_WAY_IN]));
    renderPage({ result: { kind: "error", error: "way_in_belongs_to_another_account" } });
    expect(
      await screen.findByText("That account already signs in to another Campbooks account."),
    ).toBeVisible();
  });

  it("marks a way in added this week and says what to do if it wasn't you", async () => {
    onOctober14();
    mockApi(listed([GOOGLE_WAY_IN, ZOHO_WAY_IN]));
    renderPage();

    const list = await screen.findByRole("list", { name: "Your ways to sign in" });
    const [google, zoho] = within(list).getAllByRole("listitem");
    if (google === undefined || zoho === undefined) {
      throw new Error("two rows expected");
    }
    expect(within(zoho).getByText("New")).toBeVisible();
    expect(within(google).queryByText("New")).toBeNull();
    const notice = screen.getByRole("status", { name: "" });
    expect(within(notice).getByText("A way to sign in was added this week")).toBeVisible();
    expect(within(notice).getByText(/remove it, then sign out everywhere/u)).toBeVisible();
  });

  it("never calls the passkey an account was made with a new way in", async () => {
    onOctober14();
    mockApi({ ...listed([]), [PASSKEYS]: { body: { items: [SIGN_UP_PASSKEY] } } });
    renderPage({ sections: [PasskeysSection] });
    expect(await screen.findByText("Signed up with")).toBeVisible();
    expect(screen.queryByText("New")).toBeNull();
    expect(screen.queryByText("A way to sign in was added this week")).toBeNull();
  });

  it("says nothing when every way in is older than a week", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-11-30T12:00:00Z"));
    mockApi(listed([GOOGLE_WAY_IN, ZOHO_WAY_IN]));
    renderPage();
    await screen.findByRole("list", { name: "Your ways to sign in" });
    expect(screen.queryByText("New")).toBeNull();
    expect(screen.queryByText("A way to sign in was added this week")).toBeNull();
  });

  it("signs out everywhere after confirming in place, then this browser", async () => {
    const api = mockApi({
      ...listed([GOOGLE_WAY_IN]),
      [EVERYWHERE]: { status: 204 },
      [LOGOUT]: { body: { logout_url: null } },
    });
    const leave = renderPage();

    const open = await screen.findByRole("button", { name: "Sign out everywhere" });
    expect(open).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(open);
    expect(open).toHaveAttribute("aria-expanded", "true");
    expect(
      screen.getByText(/on every device and in every app, this browser included/u),
    ).toBeVisible();
    expect(api.calls.map((call) => call.route)).not.toContain(EVERYWHERE);
    fireEvent.click(screen.getByRole("button", { name: "Yes, sign out everywhere" }));
    await waitFor(() => {
      expect(leave).toHaveBeenCalledWith("/");
    });
    const routes = api.calls.map((call) => call.route);
    expect(routes.indexOf(EVERYWHERE)).toBeLessThan(routes.indexOf(LOGOUT));
  });

  it("keeps you signed in when signing out everywhere fails", async () => {
    mockApi({ ...listed([GOOGLE_WAY_IN]), [EVERYWHERE]: problem(503, "unavailable") });
    const leave = renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Sign out everywhere" }));
    fireEvent.click(screen.getByRole("button", { name: "Yes, sign out everywhere" }));
    expect(await screen.findByText(/Couldn’t sign out everywhere/u)).toBeVisible();
    expect(leave).not.toHaveBeenCalled();
  });

  it("renders the sections other ways in bring", async () => {
    mockApi(listed([GOOGLE_WAY_IN]));
    renderPage({ sections: [() => <section aria-label="Passkeys">passkeys here</section>] });
    expect(await screen.findByRole("region", { name: "Passkeys" })).toBeVisible();
  });
});
