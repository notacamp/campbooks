import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { GMX_DISCOVERY, IMAP_CONNECTION, YAHOO_DISCOVERY } from "../../../../../test/demo/identity";
import { DUARTE, SESSION } from "../../../../../test/demo/workspaces";
import {
  mockApi,
  problem,
  renderWithProviders,
  type MockHandler,
} from "../../../../../test/render";
import type { MailboxConsent } from "../../consent";
import { GUIDE_LINKS } from "../../guides";

import { ConnectMailbox } from "./ConnectMailbox";

const W = `/bff/api/v1/w/${DUARTE.id}`;
const DISCOVER = `POST ${W}/connections/imap/discover`;
const CONNECT = `POST ${W}/connections/imap`;
const GET = `GET ${W}/connections/${IMAP_CONNECTION.id}`;
const CHECK = `POST ${W}/connections/${IMAP_CONNECTION.id}/address-check`;

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

function openDialog(routes: Record<string, MockHandler>, door: MailboxConsent = consent()) {
  const calls = mockApi({ "GET /bff/session": { body: SESSION }, ...routes });
  const onClose = vi.fn();
  const rendered = renderWithProviders(
    <ConnectMailbox workspaceId={DUARTE.id} consent={door} open onClose={onClose} />,
  );
  return { calls, onClose, door, queryClient: rendered.queryClient };
}

/** Type `address` and continue. */
async function add(address: string) {
  const field = await screen.findByLabelText("Email address");
  fireEvent.change(field, { target: { value: address } });
  fireEvent.click(screen.getByRole("button", { name: /Continue/u }));
}

const passwordField = () =>
  screen.getByLabelText<HTMLInputElement>(/password/iu, { selector: "input" });

/** The password field, once the app-password step is shown. */
const findPasswordField = () =>
  screen.findByLabelText<HTMLInputElement>(/password/iu, { selector: "input" });

/** Item `index` of `items`, or a failed test. */
function nth<T>(items: readonly T[], index: number): T {
  const item = items[index];
  if (item === undefined) {
    throw new Error(`no item ${String(index)}`);
  }
  return item;
}

describe("ConnectMailbox", () => {
  it("asks for the address first, with Enter on Continue and the address field focused", async () => {
    openDialog({});
    const field = await screen.findByLabelText("Email address");
    expect(field).toHaveFocus();
    expect(field).toHaveAttribute("autocomplete", "email");
    expect(screen.getByRole("button", { name: /Continue/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "Enter",
    );
    expect(screen.queryByLabelText(/password/iu)).toBeNull();
  });

  it("refuses something that is not an address without asking the server", async () => {
    const { calls } = openDialog({});
    await add("ana");
    expect(await screen.findByText(/Enter a valid email address/u)).toBeVisible();
    expect(calls.calls.map((call) => call.route)).not.toContain(DISCOVER);
  });

  it("sends a Gmail address to Google's own sign-in", async () => {
    const { door } = openDialog({
      [DISCOVER]: { body: { kind: "use_provider", provider: "google" } },
    });
    await add("ana@gmail.com");

    expect(await screen.findByRole("heading", { name: "Connect Gmail" })).toBeVisible();
    expect(screen.queryByLabelText(/password/iu)).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Also use this account to sign in" }));
    fireEvent.click(screen.getByRole("button", { name: /Connect Gmail/u }));
    expect(door.start).toHaveBeenCalledWith({
      purpose: "connect",
      provider: "google",
      signIn: false,
    });
  });

  it("sends a Zoho address to Zoho's sign-in", async () => {
    const { door } = openDialog({
      [DISCOVER]: { body: { kind: "use_provider", provider: "zoho" } },
    });
    await add("ana@zohomail.com");

    fireEvent.click(await screen.findByRole("button", { name: /Connect Zoho Mail/u }));
    expect(door.start).toHaveBeenCalledWith({ purpose: "connect", provider: "zoho", signIn: true });
  });

  it("says a provider this instance does not offer is not set up", async () => {
    openDialog(
      { [DISCOVER]: { body: { kind: "use_provider", provider: "zoho" } } },
      consent({ providers: ["google"] }),
    );
    await add("ana@zohomail.com");

    expect(await screen.findByText(/doesn't have Zoho Mail set up yet/u)).toBeVisible();
    expect(screen.queryByRole("button", { name: /Connect Zoho Mail/u })).toBeNull();
  });

  it("says Outlook is coming soon, and connects nothing", async () => {
    openDialog({ [DISCOVER]: { body: { kind: "not_supported_yet", provider: "microsoft" } } });
    await add("ana@outlook.com");

    expect(await screen.findByRole("heading", { name: "Outlook is coming soon" })).toBeVisible();
    expect(screen.queryByLabelText(/password/iu)).toBeNull();
    expect(screen.queryByRole("button", { name: /^Connect/u })).toBeNull();
  });

  it("sends an Outlook address to Microsoft's sign-in where the instance offers it", async () => {
    const { door } = openDialog(
      { [DISCOVER]: { body: { kind: "use_provider", provider: "microsoft" } } },
      consent({ providers: ["google", "microsoft", "zoho"] }),
    );
    await add("ana@outlook.com");

    expect(await screen.findByRole("heading", { name: "Connect Outlook" })).toBeVisible();
    expect(screen.getByText(/is an Outlook or Microsoft 365 mailbox/u)).toBeVisible();
    expect(screen.queryByText("Outlook is coming soon")).toBeNull();
    expect(screen.queryByLabelText(/password/iu)).toBeNull();
    expect(
      screen.getByRole("checkbox", { name: "Also use this account to sign in" }),
    ).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: /Connect Outlook/u }));
    expect(door.start).toHaveBeenCalledWith({
      purpose: "connect",
      provider: "microsoft",
      signIn: true,
    });
  });

  it("says Outlook is not set up where Microsoft's sign-in is missing", async () => {
    openDialog({ [DISCOVER]: { body: { kind: "use_provider", provider: "microsoft" } } });
    await add("ana@outlook.com");

    expect(await screen.findByText(/doesn't have Outlook set up yet/u)).toBeVisible();
    expect(screen.queryByRole("button", { name: /Connect Outlook/u })).toBeNull();
  });

  it("shows Yahoo's app-password steps, with Advanced closed and prefilled", async () => {
    openDialog({ [DISCOVER]: { body: YAHOO_DISCOVERY } });
    await add("ana@yahoo.example");

    const steps = await screen.findByRole("list", {
      name: "How to get an app password at Yahoo",
    });
    expect(within(steps).getAllByRole("listitem")).toHaveLength(3);
    expect(screen.getByRole("link", { name: /Yahoo's help page/u })).toHaveAttribute(
      "href",
      "https://help.yahoo.com/kb/SLN15241.html",
    );
    expect(screen.getByText(/Yahoo needs an app password/u)).toBeVisible();
    const field = passwordField();
    expect(field).toHaveAttribute("type", "password");
    expect(field).toHaveAttribute("autocomplete", "new-password");
    await waitFor(() => {
      expect(field).toHaveFocus();
    });
    const advanced = screen.getByText("Advanced: username and servers").closest("details");
    expect(advanced).not.toHaveAttribute("open");
    expect(within(advanced as HTMLElement).getAllByLabelText("Server")[0]).toHaveValue(
      "imap.mail.yahoo.com",
    );
  });

  it("tells GMX members to turn on IMAP first, with GMX's own pages", async () => {
    openDialog({ [DISCOVER]: { body: GMX_DISCOVERY } });
    await add("ana@gmx.example");

    expect(await screen.findByText(/Turn on IMAP first/u)).toBeVisible();
    expect(screen.getByRole("link", { name: /Turning on IMAP at GMX/u })).toHaveAttribute(
      "href",
      "https://hilfe.gmx.net/pop-imap/einschalten.html",
    );
  });

  it("opens Advanced, empty, when no servers were found", async () => {
    const { calls } = openDialog({
      [DISCOVER]: { body: { kind: "not_found" } },
      [CONNECT]: { status: 201, body: { ...IMAP_CONNECTION, address_verified: true } },
    });
    await add("ana@bakery.example");

    const advanced = (await screen.findByText("Advanced: username and servers")).closest("details");
    expect(advanced).toHaveAttribute("open");
    expect(screen.getByLabelText("Username")).toHaveValue("ana@bakery.example");
    // Servers are needed: none, and connecting asks for them.
    fireEvent.change(passwordField(), { target: { value: "app-pass" } });
    fireEvent.click(screen.getByRole("button", { name: /^Connect/u }));
    expect(await screen.findByText(/Check the servers/u)).toBeVisible();
    const hosts = screen.getAllByLabelText("Server");
    fireEvent.change(nth(hosts, 0), { target: { value: "mail.bakery.example" } });
    fireEvent.change(nth(hosts, 1), { target: { value: "mail.bakery.example" } });
    fireEvent.change(passwordField(), { target: { value: "app-pass" } });
    fireEvent.click(screen.getByRole("button", { name: /^Connect/u }));
    await waitFor(() => {
      expect(calls.calls.map((call) => call.route)).toContain(CONNECT);
    });
    const sent = calls.calls.find((call) => call.route === CONNECT)?.body;
    expect(sent).toEqual({
      address: "ana@bakery.example",
      password: "app-pass",
      servers: {
        imap: { host: "mail.bakery.example", port: 993, security: "tls" },
        smtp: { host: "mail.bakery.example", port: 465, security: "tls" },
      },
    });
  });

  it("offers only TLS or STARTTLS", async () => {
    openDialog({ [DISCOVER]: { body: YAHOO_DISCOVERY } });
    await add("ana@yahoo.example");

    const security = nth(await screen.findAllByLabelText("Security"), 0);
    const options = within(security)
      .getAllByRole("option")
      .map((option) => option.textContent);
    expect(options).toEqual(["TLS", "STARTTLS"]);
  });

  it("explains a refused app password, and never keeps the password", async () => {
    const { queryClient } = openDialog({
      [DISCOVER]: { body: YAHOO_DISCOVERY },
      [CONNECT]: problem(400, "imap_login_refused"),
    });
    await add("ana@yahoo.example");

    fireEvent.change(await findPasswordField(), { target: { value: "s3cret-pass" } });
    fireEvent.click(screen.getByRole("button", { name: /^Connect/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Yahoo needs an app password, not your usual password",
    );
    expect(screen.queryByText(/Localized detail/u)).toBeNull();
    expect(passwordField()).toHaveValue("");
    const cached = JSON.stringify(
      queryClient
        .getMutationCache()
        .getAll()
        .map((mutation) => mutation.state.variables),
    );
    expect(cached).not.toContain("s3cret-pass");
  });

  it.each([
    ["imap_unreachable", "couldn't reach the mail server"],
    ["starttls_missing", "doesn't offer an encrypted connection"],
    ["host_refused", "on a private network"],
    ["too_many_attempts", "Too many failed sign-ins"],
    ["imap_not_an_imap_server", "aren't an IMAP mail server"],
    ["made_up_code", "The mailbox wasn't connected"],
  ])("explains %s in plain words", async (code, text) => {
    openDialog({ [DISCOVER]: { body: YAHOO_DISCOVERY }, [CONNECT]: problem(400, code) });
    await add("ana@yahoo.example");

    fireEvent.change(await findPasswordField(), { target: { value: "pw" } });
    fireEvent.click(screen.getByRole("button", { name: /^Connect/u }));
    expect(await screen.findByRole("alert")).toHaveTextContent(text);
  });

  it("asks to sign in again when the sign-in is too old", async () => {
    const { door } = openDialog({ [DISCOVER]: problem(403, "step_up_required") });
    await add("ana@yahoo.example");

    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("Sign in again first")).toBeVisible();
    fireEvent.click(within(alert).getByRole("button", { name: "Sign in again" }));
    expect(door.signInAgain).toHaveBeenCalled();
  });

  it("offers the instance's providers when IMAP is off", async () => {
    const { door } = openDialog({ [DISCOVER]: problem(409, "provider_not_configured") });
    await add("ana@yahoo.example");

    fireEvent.click(await screen.findByRole("button", { name: /Connect Zoho Mail/u }));
    expect(door.start).toHaveBeenCalledWith({ purpose: "connect", provider: "zoho", signIn: true });
  });

  it("follows the address check until it is verified", async () => {
    openDialog({
      [DISCOVER]: { body: YAHOO_DISCOVERY },
      [CONNECT]: { status: 201, body: IMAP_CONNECTION },
      [GET]: { body: { ...IMAP_CONNECTION, address_verified: true, address_check: null } },
    });
    await add("ana@bakery.example");
    fireEvent.change(await findPasswordField(), { target: { value: "pw" } });
    fireEvent.click(screen.getByRole("button", { name: /^Connect/u }));

    expect(await screen.findByText("Verified")).toBeVisible();
    expect(screen.getByText(/can send mail from Campbooks/u)).toBeVisible();
  });

  it("says sending stays off when the check failed, and tries again", async () => {
    let checked = false;
    const { calls } = openDialog({
      [DISCOVER]: { body: YAHOO_DISCOVERY },
      [CONNECT]: { status: 201, body: IMAP_CONNECTION },
      [GET]: () => ({
        body: { ...IMAP_CONNECTION, address_check: checked ? "checking" : "failed" },
      }),
      [CHECK]: () => {
        checked = true;
        return { status: 202, body: IMAP_CONNECTION };
      },
    });
    await add("ana@bakery.example");
    fireEvent.change(await findPasswordField(), { target: { value: "pw" } });
    fireEvent.click(screen.getByRole("button", { name: /^Connect/u }));

    expect(
      await screen.findByText("We couldn't verify this address: sending stays off"),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Checking your address…")).toBeVisible();
    expect(calls.calls.map((call) => call.route)).toContain(CHECK);
  });

  it("goes back to the address, and closes on Escape", async () => {
    const { onClose } = openDialog({ [DISCOVER]: { body: YAHOO_DISCOVERY } });
    await add("ana@yahoo.example");

    await screen.findByRole("list", { name: /app password at Yahoo/u });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByLabelText("Email address")).toHaveValue("ana@yahoo.example");
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("links only to the providers' own HTTPS help pages", () => {
    const hosts = new Set(GUIDE_LINKS.map((link) => new URL(link).host));
    expect(GUIDE_LINKS.every((link) => link.startsWith("https://"))).toBe(true);
    expect([...hosts].toSorted()).toEqual([
      "help.aol.com",
      "help.yahoo.com",
      "hilfe.gmx.net",
      "hilfe.web.de",
      "support.apple.com",
      "support.gmx.com",
      "www.fastmail.help",
      "yandex.com",
    ]);
  });
});
