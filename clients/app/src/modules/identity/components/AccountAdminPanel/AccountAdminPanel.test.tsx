import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  ADMIN_ANA_ACCOUNT,
  ADMIN_ANA_SUSPENDED,
  NOT_FOUND_PROBLEM,
} from "../../../../../test/demo/admin";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { AccountAdminPanel } from "./AccountAdminPanel";

const ACCOUNT = `/bff/admin/v1/identity/accounts/${ADMIN_ANA_ACCOUNT.id}`;

/** The value next to `label`. */
function valueOf(label: string) {
  const term = screen.getByText(label, { selector: "dt" });
  const row = term.parentElement;
  if (row === null) {
    throw new Error(`no row for ${label}`);
  }
  return within(row).getByRole("definition");
}

function renderPanel() {
  return renderWithProviders(
    <AccountAdminPanel accountId={ADMIN_ANA_ACCOUNT.id} name="Ana Duarte" />,
  );
}

describe("AccountAdminPanel", () => {
  it("says whether and how the person signs in, without anything secret", async () => {
    mockApi({ [`GET ${ACCOUNT}`]: { body: ADMIN_ANA_ACCOUNT } });
    renderPanel();

    expect(await screen.findByText("Can sign in")).toBeVisible();
    expect(valueOf("Primary email address")).toHaveTextContent(
      "ana@example.comVerified by the provider",
    );
    expect(valueOf("Signed in now")).toHaveTextContent("On 2 browsers");
    expect(valueOf("Passkeys")).toHaveTextContent("1 passkey");
    const ways = screen.getByRole("list", { name: "Ways to sign in" });
    expect(within(ways).getByText("Google")).toBeVisible();
    expect(within(ways).getByText(/ana@example\.com · Added/u)).toBeVisible();
  });

  it("suspends only after the confirmation says what happens", async () => {
    const api = mockApi({
      [`GET ${ACCOUNT}`]: { body: ADMIN_ANA_ACCOUNT },
      [`POST ${ACCOUNT}/suspend`]: { body: ADMIN_ANA_SUSPENDED },
    });
    renderPanel();

    fireEvent.click(await screen.findByRole("button", { name: "Suspend…" }));
    const confirm = screen.getByRole("group", { name: /Suspend Ana Duarte\?/u });
    expect(confirm).toHaveTextContent("signed out everywhere at once");
    expect(confirm).toHaveTextContent("stay as they are");
    expect(within(confirm).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(api.calls.some((call) => call.route.endsWith("/suspend"))).toBe(false);

    fireEvent.click(within(confirm).getByRole("button", { name: "Suspend" }));

    expect(
      await screen.findByText("Ana Duarte is suspended and signed out everywhere."),
    ).toBeVisible();
    expect(valueOf("Status")).toHaveTextContent("Suspended");
    expect(screen.getByRole("button", { name: "Unsuspend" })).toBeVisible();
  });

  it("explains that an administrator's profile must be revoked first", async () => {
    mockApi({
      [`GET ${ACCOUNT}`]: { body: ADMIN_ANA_ACCOUNT },
      [`POST ${ACCOUNT}/suspend`]: problem(409, "account_is_admin"),
    });
    renderPanel();

    fireEvent.click(await screen.findByRole("button", { name: "Suspend…" }));
    fireEvent.click(screen.getByRole("button", { name: "Suspend" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("This person is an administrator.");
    expect(alert).toHaveTextContent("campbooks-server admin revoke");
    expect(valueOf("Status")).toHaveTextContent("Can sign in");
  });

  it("unsuspends a suspended account at once", async () => {
    const api = mockApi({
      [`GET ${ACCOUNT}`]: { body: ADMIN_ANA_SUSPENDED },
      [`POST ${ACCOUNT}/unsuspend`]: { body: ADMIN_ANA_ACCOUNT },
    });
    renderPanel();

    expect(await screen.findByText("Suspended")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Unsuspend" }));

    expect(await screen.findByText("Ana Duarte can sign in again.")).toBeVisible();
    expect(api.calls.map((call) => call.route)).toContain(`POST ${ACCOUNT}/unsuspend`);
  });

  it("says when this instance has no account to show", async () => {
    mockApi({ [`GET ${ACCOUNT}`]: { status: 404, body: NOT_FOUND_PROBLEM } });
    renderPanel();

    expect(await screen.findByText(/another identity provider/u)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Suspend…" })).toBeNull();
  });
});
