import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LAPTOP_PASSKEY, PHONE_PASSKEY } from "../../../../../test/demo/identity";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";
import {
  ATTESTATION_JSON,
  CREATION_OPTIONS,
  domError,
  stubWebAuthn,
} from "../../../../../test/webauthn";

import { PasskeysSection } from "./PasskeysSection";

const LIST = "GET /bff/api/v1/identity/passkeys";
const START = "POST /bff/api/v1/identity/passkeys/registrations";
const REGISTER = "POST /bff/api/v1/identity/passkeys";
const PASSKEY = (id: string) => `/bff/api/v1/identity/passkeys/${id}`;
const HERE = "/w/w1/settings/sign-in";

function renderSection() {
  const leave = vi.fn();
  renderWithProviders(<PasskeysSection returnTo={HERE} leave={leave} />);
  return leave;
}

describe("PasskeysSection", () => {
  it("lists your passkeys with where they live and when they were used", async () => {
    stubWebAuthn();
    mockApi({ [LIST]: { body: { items: [LAPTOP_PASSKEY, PHONE_PASSKEY] } } });
    renderSection();

    const list = await screen.findByRole("list", { name: "Your passkeys" });
    const [laptop, phone] = within(list).getAllByRole("listitem");
    if (laptop === undefined || phone === undefined) {
      throw new Error("two rows expected");
    }
    expect(within(laptop).getByText("Passkey on Mac")).toBeVisible();
    expect(
      within(laptop).getByText(/Synced across your devices · Added Oct 2, 2026/u),
    ).toBeVisible();
    expect(within(phone).getByText(/On one device · Added Oct 5, 2026$/u)).toBeVisible();
    // N then P: a sequence, shown on the button (aria-keyshortcuts can't express one).
    expect(screen.getByRole("button", { name: /Add a passkey/u })).toBeVisible();
  });

  it("adds a passkey made on this device, named after it", async () => {
    const browser = stubWebAuthn();
    vi.spyOn(globalThis.navigator, "userAgent", "get").mockReturnValue(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    );
    const added = { ...PHONE_PASSKEY, id: "p3", name: "Passkey on Windows" };
    const api = mockApi({
      [LIST]: { body: { items: [] } },
      [START]: { status: 201, body: CREATION_OPTIONS },
      [REGISTER]: { status: 201, body: added },
    });
    renderSection();

    expect(await screen.findByText("No passkeys yet.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Add a passkey/u }));
    expect(await screen.findByText(/Passkey on Windows is added/u)).toBeVisible();
    expect(screen.getByRole("list", { name: "Your passkeys" })).toBeVisible();
    expect(api.calls.at(-1)?.body).toEqual({
      credential: ATTESTATION_JSON,
      name: "Passkey on Windows",
    });
    expect(browser.create).toHaveBeenCalledTimes(1);
  });

  it("asks for a fresh sign-in when the last one is too old", async () => {
    stubWebAuthn();
    mockApi({
      [LIST]: { body: { items: [LAPTOP_PASSKEY] } },
      [START]: problem(403, "step_up_required"),
    });
    const leave = renderSection();
    await screen.findByRole("list", { name: "Your passkeys" });
    fireEvent.keyDown(document.body, { key: "n" });
    fireEvent.keyDown(document.body, { key: "p" });
    expect(await screen.findByText("Sign in again first")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    expect(leave).toHaveBeenCalledWith(
      `/bff/login?return_to=${encodeURIComponent(HERE)}&max_age=600`,
    );
  });

  it("asks for a sign-in now when adding a passkey needs a fresher one", async () => {
    stubWebAuthn();
    mockApi({
      [LIST]: { body: { items: [LAPTOP_PASSKEY] } },
      [START]: problem(403, "fresh_sign_in_required"),
    });
    const leave = renderSection();
    await screen.findByRole("list", { name: "Your passkeys" });
    fireEvent.click(screen.getByRole("button", { name: "Add a passkey" }));
    expect(await screen.findByText(/sign-in from the last five minutes/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    expect(leave).toHaveBeenCalledWith(
      `/bff/login?return_to=${encodeURIComponent(HERE)}&max_age=0`,
    );
  });

  it("explains why a passkey wasn't added", async () => {
    const browser = stubWebAuthn();
    mockApi({
      [LIST]: { body: { items: [LAPTOP_PASSKEY] } },
      [START]: { status: 201, body: CREATION_OPTIONS },
      [REGISTER]: problem(409, "passkey_expired"),
    });
    renderSection();
    const add = await screen.findByRole("button", { name: /Add a passkey/u });
    await waitFor(() => {
      expect(add).toBeEnabled();
    });

    browser.create.mockRejectedValueOnce(domError("NotAllowedError"));
    fireEvent.click(add);
    expect(await screen.findByText("No passkey was added.")).toBeVisible();

    browser.create.mockRejectedValueOnce(domError("InvalidStateError"));
    fireEvent.click(add);
    expect(
      await screen.findByText("This device already has a passkey for your account."),
    ).toBeVisible();

    fireEvent.click(add);
    expect(await screen.findByText("That took too long.")).toBeVisible();

    mockApi({
      [START]: { status: 201, body: CREATION_OPTIONS },
      [REGISTER]: problem(403, "passkey_not_accepted"),
    });
    fireEvent.click(add);
    expect(await screen.findByText("That passkey wasn’t accepted.")).toBeVisible();

    mockApi({ [START]: problem(500, "internal_error") });
    fireEvent.click(add);
    expect(await screen.findByText("Couldn’t add a passkey.")).toBeVisible();
  });

  it("renames a passkey in place", async () => {
    stubWebAuthn();
    const api = mockApi({
      [LIST]: { body: { items: [LAPTOP_PASSKEY] } },
      [`PATCH ${PASSKEY(LAPTOP_PASSKEY.id)}`]: (_request, body) => ({
        body: { ...LAPTOP_PASSKEY, name: (body as { name: string }).name },
      }),
    });
    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Rename Passkey on Mac" }));
    const field = screen.getByRole("textbox", { name: "Name" });
    expect(field).toHaveFocus();
    fireEvent.change(field, { target: { value: " " } });
    fireEvent.submit(field);
    expect(screen.getByText("Couldn’t rename it.")).toBeVisible();
    fireEvent.change(field, { target: { value: "Work laptop " } });
    fireEvent.submit(field);
    expect(await screen.findByText("Work laptop")).toBeVisible();
    expect(api.calls.at(-1)?.body).toEqual({ name: "Work laptop" });
    expect(screen.queryByText("Couldn’t rename it.")).toBeNull();
  });

  it("removes a passkey only after confirming, never the last way in", async () => {
    stubWebAuthn();
    let refuse = true;
    mockApi({
      [LIST]: { body: { items: [LAPTOP_PASSKEY, PHONE_PASSKEY] } },
      [`DELETE ${PASSKEY(PHONE_PASSKEY.id)}`]: () =>
        refuse ? problem(409, "last_way_in") : { status: 204 },
    });
    renderSection();
    const remove = await screen.findByRole("button", { name: "Remove Phone" });
    fireEvent.click(remove);
    expect(screen.getByText(/Stop signing in with Phone\?/u)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByText("This is your only way to sign in.")).toBeVisible();

    refuse = false;
    fireEvent.click(screen.getByRole("button", { name: "Remove Phone" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => {
      expect(screen.queryByText("Phone")).toBeNull();
    });
  });

  it("asks for a fresh sign-in to remove one when the last is too old", async () => {
    stubWebAuthn();
    mockApi({
      [LIST]: { body: { items: [LAPTOP_PASSKEY] } },
      [`DELETE ${PASSKEY(LAPTOP_PASSKEY.id)}`]: problem(403, "step_up_required"),
    });
    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Remove Passkey on Mac" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByText("Sign in again first")).toBeVisible();
  });

  it("says when the list can't load, and when the browser can't make passkeys", async () => {
    mockApi({ [LIST]: problem(503, "unavailable") });
    renderSection();
    expect(await screen.findByText("Couldn’t load your passkeys.")).toBeVisible();
    expect(screen.getByText(/This browser can’t make passkeys/u)).toBeVisible();
  });
});
