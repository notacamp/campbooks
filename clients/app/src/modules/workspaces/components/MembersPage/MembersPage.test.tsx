import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  ANA,
  DUARTE,
  EXPIRED,
  HOUSEHOLD,
  PENDING,
  RUI,
  SESSION,
  SOFIA,
} from "../../../../../test/demo/workspaces";
import { mockApi, problem, renderWithProviders } from "../../../../../test/render";

import { MembersPage } from "./MembersPage";

const W = `/bff/api/v1/w/${DUARTE.id}`;

function routes(extra: Record<string, Parameters<typeof mockApi>[0][string]> = {}) {
  return {
    "GET /bff/session": { body: SESSION },
    [`GET ${W}/members`]: { body: { members: [ANA, SOFIA, RUI] } },
    [`GET ${W}/invitations`]: { body: { invitations: [PENDING, EXPIRED] } },
    ...extra,
  };
}

async function rowOf(name: string) {
  const list = await screen.findByRole("list", { name: `Members of ${DUARTE.name}` });
  const row = within(list)
    .getAllByRole("listitem")
    .find((item) => item.textContent.includes(name));
  if (row === undefined) {
    throw new Error(`no row for ${name}`);
  }
  return within(row);
}

describe("MembersPage", () => {
  it("lists members with roles, marks you, and lists pending invitations", async () => {
    mockApi(routes());
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    const ana = await rowOf("Ana Duarte");
    await waitFor(() => {
      expect(ana.getByText("You")).toBeVisible();
    });
    expect(ana.queryByRole("button", { name: "Remove Ana Duarte" })).toBeNull();
    const sofia = await rowOf("Sofia Pessoa");
    expect(sofia.getByRole("combobox", { name: "Role of Sofia Pessoa" })).toHaveValue("admin");

    const pending = await screen.findByRole("list", { name: "Pending invitations" });
    expect(within(pending).getByText("marta@example.com")).toBeVisible();
    expect(
      within(pending).getByRole("button", { name: "Invite joao@example.com again" }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: /Invite someone/u })).toHaveAttribute(
      "aria-keyshortcuts",
      "I",
    );
  });

  it("changes a role at once and undoes it", async () => {
    const api = mockApi(
      routes({
        [`PATCH ${W}/members/${RUI.user_id}`]: (_request, body) => ({
          body: { ...RUI, role: (body as { role: string }).role },
        }),
      }),
    );
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    const rui = await rowOf("Rui Matos");
    fireEvent.change(rui.getByRole("combobox", { name: "Role of Rui Matos" }), {
      target: { value: "admin" },
    });
    const undo = await screen.findByRole("button", { name: /Undo/u });
    expect(screen.getByText("Rui Matos: Admin")).toBeVisible();
    fireEvent.click(undo);

    await waitFor(() => {
      expect(
        api.calls.filter((call) => call.route.startsWith("PATCH")).map((call) => call.body),
      ).toEqual([{ role: "admin" }, { role: "member" }]);
    });
  });

  it("explains the last owner refusal in the row", async () => {
    mockApi(routes({ [`PATCH ${W}/members/${SOFIA.user_id}`]: problem(409, "last_owner") }));
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    const sofia = await rowOf("Sofia Pessoa");
    fireEvent.change(sofia.getByRole("combobox", { name: "Role of Sofia Pessoa" }), {
      target: { value: "member" },
    });

    expect(await sofia.findByRole("alert")).toHaveTextContent(
      "A workspace needs at least one owner.",
    );
  });

  it("removes a member only after confirming inline", async () => {
    const api = mockApi(routes({ [`DELETE ${W}/members/${RUI.user_id}`]: { status: 204 } }));
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    const rui = await rowOf("Rui Matos");
    fireEvent.click(rui.getByRole("button", { name: "Remove Rui Matos" }));
    const confirm = rui.getByRole("group", { name: /Remove Rui Matos from Duarte & Pessoa\?/u });
    fireEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(api.calls.some((call) => call.route.startsWith("DELETE"))).toBe(false);

    fireEvent.click(rui.getByRole("button", { name: "Remove Rui Matos" }));
    fireEvent.click(within(rui.getByRole("group")).getByRole("button", { name: "Remove" }));

    await waitFor(() => {
      expect(screen.queryByText("Rui Matos")).toBeNull();
    });
  });

  it("says when removing is not allowed", async () => {
    mockApi(
      routes({ [`DELETE ${W}/members/${SOFIA.user_id}`]: { status: 403, body: { status: 403 } } }),
    );
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    const sofia = await rowOf("Sofia Pessoa");
    fireEvent.click(sofia.getByRole("button", { name: "Remove Sofia Pessoa" }));
    fireEvent.click(within(sofia.getByRole("group")).getByRole("button", { name: "Remove" }));

    expect(await sofia.findByRole("alert")).toHaveTextContent(
      "Only owners can change or remove an owner.",
    );
  });

  it("invites by email, from the keyboard too", async () => {
    const api = mockApi(
      routes({
        [`POST ${W}/invitations`]: (_request, body) => {
          const { email } = body as { email: string };
          return email === "rui@example.com"
            ? problem(409, "already_a_member")
            : {
                status: 201,
                body: {
                  ...PENDING,
                  id: "new",
                  email,
                  role: "admin",
                  accept_url: "https://campbooks.example.com/invitations/accept?token=demo",
                },
              };
        },
      }),
    );
    const { queryClient } = renderWithProviders(<MembersPage workspace={DUARTE} />);
    await rowOf("Ana Duarte");

    fireEvent.keyDown(document.body, { key: "i" });
    const email = screen.getByRole("textbox", { name: "Email" });
    expect(email).toHaveFocus();

    fireEvent.change(email, { target: { value: "bea@" } });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));
    expect(email).toHaveAccessibleDescription(/Enter a full email address/u);

    fireEvent.change(email, { target: { value: "rui@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));
    await waitFor(() => {
      expect(email).toHaveAccessibleDescription("rui@example.com is already a member.");
    });

    fireEvent.change(email, { target: { value: "bea@example.com" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Role" }), {
      target: { value: "admin" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Invite" }));

    expect(await screen.findByText("Invited bea@example.com as Admin.")).toBeVisible();
    expect(api.calls.at(-1)?.body).toEqual({ email: "bea@example.com", role: "admin" });
    const pending = screen.getByRole("list", { name: "Pending invitations" });
    expect(within(pending).getByText("bea@example.com")).toBeVisible();

    // The link is shown once, to copy and send on (C copies it).
    const link = screen.getByRole("textbox", { name: "Invitation link for bea@example.com" });
    expect(link).toHaveValue("https://campbooks.example.com/invitations/accept?token=demo");
    expect(screen.getByText(/doesn’t email invitations yet/u)).toBeVisible();
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    fireEvent.keyDown(document.body, { key: "c" });
    expect(await screen.findByText(/Link copied/u)).toBeVisible();
    expect(writeText).toHaveBeenCalledWith(
      "https://campbooks.example.com/invitations/accept?token=demo",
    );
    // The cached invitation list never keeps the link.
    const cached = queryClient
      .getQueryCache()
      .getAll()
      .map((query) => JSON.stringify(query.state.data));
    expect(cached.join("\n")).not.toContain("token=demo");
  });

  it("invites an expired invitation again", async () => {
    const api = mockApi(
      routes({
        [`POST ${W}/invitations`]: {
          status: 201,
          body: {
            ...EXPIRED,
            expires_at: PENDING.expires_at,
            accept_url: "https://campbooks.example.com/invitations/accept?token=fresh",
          },
        },
      }),
    );
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    fireEvent.click(await screen.findByRole("button", { name: "Invite joao@example.com again" }));

    expect(
      await screen.findByRole("textbox", { name: "Invitation link for joao@example.com" }),
    ).toHaveValue("https://campbooks.example.com/invitations/accept?token=fresh");
    expect(api.calls.at(-1)?.body).toEqual({ email: "joao@example.com", role: "admin" });
  });

  it("shows members read-only to a member", async () => {
    const api = mockApi(routes());
    renderWithProviders(<MembersPage workspace={HOUSEHOLD} />);

    expect(
      await screen.findByText("Only owners and admins can invite people or change roles."),
    ).toBeVisible();
    expect(screen.queryByRole("textbox", { name: "Email" })).toBeNull();
    expect(api.calls.some((call) => call.route.endsWith("/invitations"))).toBe(false);
  });

  it("offers to try again when the members can't load", async () => {
    mockApi({
      "GET /bff/session": { body: SESSION },
      [`GET ${W}/members`]: { status: 503, body: { status: 503 } },
      [`GET ${W}/invitations`]: { status: 503, body: { status: 503 } },
    });
    renderWithProviders(<MembersPage workspace={DUARTE} />);

    expect(await screen.findByRole("heading", { name: "Couldn’t load the members" })).toBeVisible();
    expect(await screen.findByText("Couldn’t load the pending invitations.")).toBeVisible();
    for (const retry of screen.getAllByRole("button", { name: "Try again" })) {
      fireEvent.click(retry);
    }
  });
});
