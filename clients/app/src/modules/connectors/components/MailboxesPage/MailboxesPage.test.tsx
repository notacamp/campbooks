import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  CONNECTION,
  SHARED_CONNECTION,
  SOFIA_CONNECTION,
  ZOHO_CONNECTION,
} from "../../../../../test/demo/identity";
import { ANA, DUARTE, HOUSEHOLD, SESSION, SOFIA } from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";
import type { MailboxConsent } from "../../consent";

import { MailboxesPage, type MailboxesPageProps } from "./MailboxesPage";

const CONNECTIONS = `GET /bff/api/v1/w/${DUARTE.id}/connections`;
const MEMBERS = `GET /bff/api/v1/w/${DUARTE.id}/members`;
const DISCONNECT = (id: string) => `DELETE /bff/api/v1/w/${DUARTE.id}/connections/${id}`;

function consent(overrides: Partial<MailboxConsent> = {}): MailboxConsent {
  return {
    providers: ["google", "zoho"],
    providersFailed: false,
    start: vi.fn(),
    starting: false,
    refusal: undefined,
    signInAgain: vi.fn(),
    wayInFate: () => "unchanged",
    ...overrides,
  };
}

function api(connections: unknown[], extra: Parameters<typeof mockApi>[0] = {}) {
  return mockApi({
    "GET /bff/session": { body: SESSION },
    [CONNECTIONS]: { body: { connections } },
    [MEMBERS]: { body: { members: [ANA, SOFIA] } },
    ...extra,
  });
}

function renderPage(props: Partial<MailboxesPageProps> = {}) {
  const door = props.consent ?? consent();
  renderWithProviders(<MailboxesPage workspace={DUARTE} consent={door} {...props} />);
  return door;
}

const rows = async () =>
  within(await screen.findByRole("list", { name: "Connected mailboxes" })).getAllByRole("listitem");

/** Row `index`, or a failed test. */
async function row(index: number): Promise<HTMLElement> {
  const found = (await rows())[index];
  if (found === undefined) {
    throw new Error(`no row ${String(index)}`);
  }
  return found;
}

describe("MailboxesPage", () => {
  it("lists each mailbox with who connected it and how it is doing", async () => {
    api([CONNECTION, ZOHO_CONNECTION, SOFIA_CONNECTION]);
    renderPage();

    const gmail = await row(0);
    const zoho = await row(1);
    const sofia = await row(2);
    expect(within(gmail).getByText("ana@example.com")).toBeVisible();
    expect(within(gmail).getByText("Gmail · Connected by you")).toBeVisible();
    expect(within(gmail).getByText("Syncing")).toBeVisible();
    expect(within(zoho).getByText("Needs you to sign in again")).toBeVisible();
    await waitFor(() => {
      expect(within(sofia).getByText("Gmail · Connected by Sofia Pessoa")).toBeVisible();
    });
    expect(within(sofia).getByText("Paused by Gmail for a few minutes")).toBeVisible();
    // Only your own mailbox offers Reconnect.
    expect(within(zoho).getByRole("button", { name: /^Reconnect/u })).toBeVisible();
    expect(within(sofia).queryByRole("button", { name: /^Reconnect/u })).toBeNull();
  });

  it("names the colleague who must sign in again", async () => {
    api([{ ...SOFIA_CONNECTION, health: "needs_reauth" }]);
    renderPage();

    expect(await screen.findByText("Needs Sofia Pessoa to sign in again")).toBeVisible();
  });

  it("connects through the front door, for each provider the server lists", async () => {
    api([CONNECTION]);
    const door = renderPage();

    fireEvent.click(await screen.findByRole("button", { name: /Connect Zoho Mail/u }));
    expect(door.start).toHaveBeenCalledWith({ purpose: "connect", provider: "zoho", signIn: true });
    expect(screen.queryByRole("button", { name: /Connect Microsoft/u })).toBeNull();
  });

  it("connects a shared mailbox without making it a way in", async () => {
    api([CONNECTION]);
    const door = renderPage();

    const box = await screen.findByRole("checkbox", { name: "Also use this account to sign in" });
    expect(box).toBeChecked();
    fireEvent.click(box);
    fireEvent.click(screen.getByRole("button", { name: /Connect Gmail/u }));
    expect(door.start).toHaveBeenCalledWith({
      purpose: "connect",
      provider: "google",
      signIn: false,
    });
  });

  it("says when no provider is set up", async () => {
    api([]);
    renderPage({ consent: consent({ providers: [] }) });

    expect(await screen.findByText("No mailboxes yet")).toBeVisible();
    expect(screen.getByText(/has no mail provider set up yet/u)).toBeVisible();
  });

  it("reconnects your mailbox that needs it, with R on the chosen row", async () => {
    api([CONNECTION, ZOHO_CONNECTION]);
    const door = renderPage();

    const zoho = await row(1);
    expect(zoho).toHaveAttribute("aria-current", "true");
    fireEvent.keyDown(document, { key: "r" });
    expect(door.start).toHaveBeenCalledWith({
      purpose: "reconnect",
      connection: ZOHO_CONNECTION.id,
    });
  });

  it("disconnects after an inline confirmation, deleting its mail by default", async () => {
    const asked: string[] = [];
    const calls = api([CONNECTION, ZOHO_CONNECTION], {
      [DISCONNECT(ZOHO_CONNECTION.id)]: (request: Request) => {
        asked.push(new URL(request.url).search);
        return { status: 204 };
      },
    });
    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: "Disconnect ana.duarte@example.net" }),
    );
    expect(screen.getByRole("radio", { name: "Delete its mail from Campbooks" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Keep its history, read-only" })).not.toBeChecked();
    expect(
      screen.getByText(/the mail stays in Zoho Mail: nothing is deleted there/u),
    ).toBeVisible();
    expect(screen.getByText(/How you sign in to Campbooks doesn’t change/u)).toBeVisible();
    expect(calls.calls.map((call) => call.route)).not.toContain(DISCONNECT(ZOHO_CONNECTION.id));
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(await screen.findByText("ana.duarte@example.net is disconnected")).toBeVisible();
    expect(screen.getByText(/Its mail is leaving Campbooks/u)).toBeVisible();
    expect(asked).toEqual(["?mail=delete"]);
    expect(await rows()).toHaveLength(1);
  });

  it("keeps focus on the page: back on Disconnect when cancelled, on the result when done", async () => {
    api([CONNECTION, ZOHO_CONNECTION], { [DISCONNECT(CONNECTION.id)]: { status: 204 } });
    renderPage();

    const button = await screen.findByRole("button", { name: "Disconnect ana@example.com" });
    fireEvent.click(button);
    fireEvent.click(screen.getByRole("button", { name: "Don’t disconnect" }));
    await waitFor(() => {
      expect(button).toHaveFocus();
    });
    fireEvent.click(button);
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    const done = await screen.findByText("ana@example.com is disconnected");
    await waitFor(() => {
      expect(done.closest("[tabindex='-1']")).toHaveFocus();
    });
  });

  it("keeps the history read-only when chosen", async () => {
    const asked: string[] = [];
    api([CONNECTION], {
      [DISCONNECT(CONNECTION.id)]: (request: Request) => {
        asked.push(new URL(request.url).search);
        return { status: 204 };
      },
    });
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Disconnect ana@example.com" }));
    fireEvent.click(screen.getByRole("radio", { name: "Keep its history, read-only" }));
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(await screen.findByText(/Its history stays here, read-only/u)).toBeVisible();
    expect(asked).toEqual(["?mail=keep"]);
  });

  it("offers no Disconnect on a colleague's mailbox, even to an owner", async () => {
    api([CONNECTION, SOFIA_CONNECTION]);
    renderPage();

    await screen.findByRole("button", { name: "Disconnect ana@example.com" });
    expect(screen.queryByRole("button", { name: "Disconnect sofia@example.com" })).toBeNull();
  });

  it("says the way in goes too when it came only from connecting", async () => {
    api([CONNECTION, ZOHO_CONNECTION]);
    renderPage({ consent: consent({ wayInFate: () => "removed" }) });

    fireEvent.click(
      await screen.findByRole("button", { name: "Disconnect ana.duarte@example.net" }),
    );
    expect(screen.getByText(/also stops being a way to sign in/u)).toBeVisible();
    // Keeping the history is the grantor's choice, whatever the mailbox.
    expect(screen.getByRole("radio", { name: "Keep its history, read-only" })).not.toBeChecked();
  });

  it("says sign-in doesn’t change for a mailbox that isn’t a way in", async () => {
    mockApi({
      "GET /bff/session": { body: SESSION },
      [`GET /bff/api/v1/w/${HOUSEHOLD.id}/connections`]: {
        body: { connections: [SHARED_CONNECTION] },
      },
      [`GET /bff/api/v1/w/${HOUSEHOLD.id}/members`]: { body: { members: [ANA] } },
    });
    renderWithProviders(
      <MailboxesPage workspace={HOUSEHOLD} consent={consent({ wayInFate: () => "unchanged" })} />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Disconnect loja@example.com" }));
    expect(screen.getByText(/How you sign in to Campbooks doesn’t change/u)).toBeVisible();
  });

  it("treats a disconnect that finds nothing as an error, not a success", async () => {
    api([CONNECTION], { [DISCONNECT(CONNECTION.id)]: problem(404, "not_found") });
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Disconnect ana@example.com" }));
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "ana@example.com wasn’t found: it may already be disconnected",
    );
    expect(screen.queryByText("ana@example.com is disconnected")).toBeNull();
  });

  it("asks to disconnect the chosen mailbox with Backspace, after J", async () => {
    api([CONNECTION, ZOHO_CONNECTION]);
    renderPage();

    const gmail = await row(0);
    fireEvent.keyDown(document, { key: "k" });
    await waitFor(() => {
      expect(gmail).toHaveAttribute("aria-current", "true");
    });
    fireEvent.keyDown(document, { key: "Backspace" });
    expect(await screen.findByText(/^Disconnect ana@example.com\?/u)).toBeVisible();
  });

  it("explains a failed disconnect in place", async () => {
    api([CONNECTION], { [DISCONNECT(CONNECTION.id)]: problem(503, "unavailable") });
    renderPage();

    fireEvent.click(await screen.findByRole("button", { name: "Disconnect ana@example.com" }));
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "ana@example.com couldn’t be disconnected",
    );
  });

  it("offers a fresh sign-in when connecting needs one", async () => {
    api([CONNECTION]);
    const door = renderPage({ consent: consent({ refusal: "step_up" }) });

    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Sign in again first")).toBeVisible();
    fireEvent.click(within(alert).getByRole("button", { name: "Sign in again" }));
    expect(door.signInAgain).toHaveBeenCalled();
  });

  it("says what came back from the front door", async () => {
    api([CONNECTION, { ...ZOHO_CONNECTION, health: "ok" }]);
    renderPage({ result: { kind: "connected", connection: ZOHO_CONNECTION.id } });

    expect(await screen.findByText("Zoho Mail ana.duarte@example.net is connected")).toBeVisible();
  });

  it("explains a refusal from the front door", async () => {
    api([CONNECTION]);
    renderPage({ result: { kind: "error", error: "account_connected_by_another_member" } });

    expect(await screen.findByRole("alert")).toHaveTextContent("Someone here already connected it");
  });

  it("tells a member they see only their own", async () => {
    mockApi({
      "GET /bff/session": { body: SESSION },
      [`GET /bff/api/v1/w/${HOUSEHOLD.id}/connections`]: { body: { connections: [] } },
      [`GET /bff/api/v1/w/${HOUSEHOLD.id}/members`]: { body: { members: [ANA] } },
    });
    renderWithProviders(<MailboxesPage workspace={HOUSEHOLD} consent={consent()} />, "pt-PT");

    expect(await screen.findByText(/Vê as caixas de correio que ligou/u)).toBeVisible();
  });
});
