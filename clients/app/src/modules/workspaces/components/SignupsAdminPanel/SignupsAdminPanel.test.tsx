import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SIGNUP_CODES, SIGNUP_REQUESTS } from "../../../../../test/demo/admin";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { endOfDay } from "./SignupCodes";
import { SignupsAdminPanel } from "./SignupsAdminPanel";

const REQUESTS = "GET /bff/admin/v1/workspaces/signup-requests";
const CODES = "GET /bff/admin/v1/workspaces/signup-codes";
const MINT = "POST /bff/admin/v1/workspaces/signup-codes";
const INES = "0192f1a0-0000-7000-8000-0000000000a3";

const list = (requests = SIGNUP_REQUESTS) => ({ body: { requests, next: null } });

describe("Admin → Signups → Requests", () => {
  it("lists who asked, waiting first, and says when nobody is waiting", async () => {
    mockApi({ [REQUESTS]: list() });
    renderWithProviders(<SignupsAdminPanel />);

    expect(await screen.findByRole("heading", { level: 1, name: "Signups" })).toBeVisible();
    const rows = await screen.findAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(within(rows[0] ?? document.body).getByText("Inês Barros")).toBeVisible();
    expect(within(rows[0] ?? document.body).getByText("Waiting")).toBeVisible();
    expect(within(rows[2] ?? document.body).getByText("Not approved")).toBeVisible();
    expect(within(rows[2] ?? document.body).getByText("Reason: Not a customer")).toBeVisible();
  });

  it("asks before approving, then approves and reloads", async () => {
    let approved = false;
    const api = mockApi({
      [REQUESTS]: () =>
        list(
          approved
            ? [
                {
                  ...(SIGNUP_REQUESTS[0] ?? fail()),
                  status: "approved",
                  decided_at: "2026-10-13T08:00:00Z",
                },
              ]
            : SIGNUP_REQUESTS,
        ),
      [`POST /bff/admin/v1/workspaces/signup-requests/${INES}/approve`]: () => {
        approved = true;
        return { body: { ...SIGNUP_REQUESTS[0], status: "approved" } };
      },
    });
    renderWithProviders(<SignupsAdminPanel />);

    fireEvent.click(await screen.findByRole("button", { name: "Approve Inês Barros" }));
    expect(
      screen.getByText(/Approve Inês Barros\? They can create their workspace/u),
    ).toBeVisible();
    // Nothing happened yet.
    expect(api.calls.filter((call) => call.route.startsWith("POST"))).toHaveLength(0);
    fireEvent.click(
      within(screen.getByRole("group", { name: /^Approve Inês Barros\?/u })).getByRole("button", {
        name: "Approve",
      }),
    );

    expect(await screen.findByText("Approved")).toBeVisible();
    expect(api.calls.filter((call) => call.route.startsWith("POST"))).toHaveLength(1);
  });

  it("rejects with an optional reason that only administrators see", async () => {
    const api = mockApi({
      [REQUESTS]: list(),
      [`POST /bff/admin/v1/workspaces/signup-requests/${INES}/reject`]: {
        body: { ...SIGNUP_REQUESTS[0], status: "rejected" },
      },
    });
    renderWithProviders(<SignupsAdminPanel />);

    fireEvent.click(await screen.findByRole("button", { name: "Reject Inês Barros" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Reason (optional)" }), {
      target: { value: "  Wrong instance " },
    });
    fireEvent.click(
      within(screen.getByRole("group", { name: /^Turn Inês Barros down\?/u })).getByRole("button", {
        name: "Reject",
      }),
    );

    await waitFor(() => {
      expect(api.calls.some((call) => call.route.startsWith("POST"))).toBe(true);
    });
    expect(api.calls.find((call) => call.route.startsWith("POST"))?.body).toEqual({
      reason: "Wrong instance",
    });
  });

  it("filters by status and shows an empty state", async () => {
    const api = mockApi({
      [REQUESTS]: (request) =>
        new URL(request.url).searchParams.get("status") === "approved" ? list([]) : list(),
    });
    renderWithProviders(<SignupsAdminPanel />);
    await screen.findAllByRole("listitem");

    fireEvent.click(screen.getByRole("button", { name: "Approved" }));
    expect(await screen.findByRole("heading", { name: "No approved requests" })).toBeVisible();
    expect(api.calls.some((call) => call.route === REQUESTS)).toBe(true);
  });

  it("pages through long lists", async () => {
    mockApi({
      [REQUESTS]: (request) => {
        const after = new URL(request.url).searchParams.get("after");
        return after === null
          ? { body: { requests: SIGNUP_REQUESTS.slice(0, 2), next: "cursor-1" } }
          : { body: { requests: SIGNUP_REQUESTS.slice(2), next: null } };
      },
    });
    renderWithProviders(<SignupsAdminPanel />);
    expect(await screen.findAllByRole("listitem")).toHaveLength(2);

    fireEvent.click(screen.getByRole("button", { name: "Show more" }));
    await waitFor(() => {
      expect(screen.getAllByRole("listitem")).toHaveLength(3);
    });
    expect(screen.queryByRole("button", { name: "Show more" })).toBeNull();
  });

  it("says when the list can't be loaded, and retries", async () => {
    mockApi({ [REQUESTS]: problem(503, "database_unavailable") });
    renderWithProviders(<SignupsAdminPanel />);

    expect(await screen.findByRole("heading", { name: "Couldn’t load this" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Try again" })).toBeVisible();
  });
});

describe("Admin → Signups → Codes", () => {
  const open = async () => {
    renderWithProviders(<SignupsAdminPanel />);
    fireEvent.click(await screen.findByRole("button", { name: "Codes" }));
  };

  it("lists codes with their use and state, never the code", async () => {
    mockApi({ [REQUESTS]: list(), [CODES]: { body: { codes: SIGNUP_CODES, next: null } } });
    await open();

    const rows = await screen.findAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(within(rows[1] ?? document.body).getByText("Team offsite")).toBeVisible();
    expect(within(rows[1] ?? document.body).getByText(/2 of 5 used/u)).toBeVisible();
    expect(within(rows[2] ?? document.body).getByText("Used up")).toBeVisible();
  });

  it("mints a code, shows it once with a copy button, then hides it", async () => {
    const api = mockApi({
      [REQUESTS]: list(),
      [CODES]: { body: { codes: [], next: null } },
      [MINT]: {
        status: 201,
        body: { code: "K7QM-2XWP-9DHE-RT4A", entry: { ...SIGNUP_CODES[0], label: "Beta, Ana" } },
      },
    });
    await open();

    fireEvent.change(await screen.findByRole("textbox", { name: "Label" }), {
      target: { value: "  Beta, Ana " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Mint code" }));

    expect(await screen.findByDisplayValue("K7QM-2XWP-9DHE-RT4A")).toBeVisible();
    expect(screen.getByRole("button", { name: "Copy code" })).toBeVisible();
    expect(api.calls.find((call) => call.route === MINT)?.body).toEqual({
      label: "Beta, Ana",
      max_uses: 1,
    });

    fireEvent.click(screen.getByRole("button", { name: "I’ve copied it" }));
    expect(screen.queryByDisplayValue("K7QM-2XWP-9DHE-RT4A")).toBeNull();
  });

  it("checks the form before asking the server", async () => {
    const api = mockApi({ [REQUESTS]: list(), [CODES]: { body: { codes: [], next: null } } });
    await open();

    fireEvent.click(await screen.findByRole("button", { name: "Mint code" }));
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveAccessibleDescription(
      "Give it a label of 1 to 100 characters.",
    );
    fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "Ana" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Uses" }), { target: { value: "0" } });
    fireEvent.click(screen.getByRole("button", { name: "Mint code" }));
    expect(screen.getByRole("spinbutton", { name: "Uses" })).toHaveAccessibleDescription(
      "Uses must be a whole number from 1 to 100000.",
    );
    expect(api.calls.some((call) => call.route === MINT)).toBe(false);
  });

  it("revokes a code after asking", async () => {
    const api = mockApi({
      [REQUESTS]: list(),
      [CODES]: { body: { codes: SIGNUP_CODES, next: null } },
      "POST /bff/admin/v1/workspaces/signup-codes/0192f1a0-0000-7000-8000-0000000000c3/revoke": {
        body: { ...SIGNUP_CODES[0], state: "revoked" },
      },
    });
    await open();

    fireEvent.click(await screen.findByRole("button", { name: "Revoke Beta, Inês" }));
    expect(screen.getByText(/Revoke “Beta, Inês”\? It stops working at once/u)).toBeVisible();
    fireEvent.click(
      within(screen.getByRole("group", { name: /^Revoke “Beta, Inês”\?/u })).getByRole("button", {
        name: "Revoke",
      }),
    );

    await waitFor(() => {
      expect(api.calls.some((call) => call.route.endsWith("/revoke"))).toBe(true);
    });
  });
});

describe("endOfDay", () => {
  it("is the end of the local day, or null for a non-day", () => {
    const end = new Date(endOfDay("2026-12-31") ?? "");
    expect([end.getFullYear(), end.getMonth(), end.getDate(), end.getHours()]).toEqual([
      2026, 11, 31, 23,
    ]);
    expect(endOfDay("")).toBeNull();
    expect(endOfDay("soon")).toBeNull();
  });
});

function fail(): never {
  throw new Error("missing demo data");
}
