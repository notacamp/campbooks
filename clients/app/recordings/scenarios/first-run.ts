import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// The first-run flow on the real app (Vite dev server), with the BFF answered
// in the browser from in-memory demo data: no server, no identity provider.
//   pnpm --dir clients dev
//   pnpm --dir clients record first-run
// Fictional people and addresses only.

interface Workspace {
  id: string;
  name: string;
  kind: "business" | "personal";
  accent: "business" | "personal" | "forest";
  role: "owner" | "admin" | "member";
  created_at: string;
}

const NOW = "2026-10-13T09:00:00Z";
const ANA = {
  user_id: "u-ana",
  display_name: "Ana Duarte",
  email: "ana@example.com",
  role: "owner",
  joined_at: NOW,
};

function fakeBff() {
  const state = {
    signedIn: false,
    workspaces: [] as Workspace[],
    members: new Map<string, Record<string, unknown>[]>(),
    invitations: new Map<string, Record<string, unknown>[]>(),
  };
  let next = 1;
  const json = (route: Route, body: unknown, status = 200) =>
    route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });

  return {
    state,
    async handle(route: Route) {
      const request = route.request();
      const url = new URL(request.url());
      const method = request.method();
      const path = url.pathname;
      const body = (request.postDataJSON() ?? {}) as Record<string, string>;

      if (path === "/bff/login") {
        state.signedIn = true;
        return route.fulfill({
          status: 303,
          headers: { location: url.searchParams.get("return_to") ?? "/" },
        });
      }
      if (path === "/bff/session") {
        return json(
          route,
          state.signedIn
            ? {
                authenticated: true,
                user: { sub: "ana", name: ANA.display_name, email: ANA.email },
              }
            : { authenticated: false },
        );
      }
      if (path === "/bff/logout") {
        state.signedIn = false;
        return json(route, { logout_url: null });
      }
      if (path === "/bff/api/v1/workspaces" && method === "GET") {
        return json(route, { workspaces: state.workspaces });
      }
      if (path === "/bff/api/v1/workspaces" && method === "POST") {
        const workspace: Workspace = {
          id: `w-${String(next++)}`,
          name: body["name"] ?? "My workspace",
          kind: body["kind"] === "personal" ? "personal" : "business",
          accent: body["kind"] === "personal" ? "personal" : "business",
          role: "owner",
          created_at: NOW,
        };
        state.workspaces.push(workspace);
        state.members.set(workspace.id, [ANA]);
        return json(route, workspace, 201);
      }
      if (path === "/bff/api/v1/workspaces/invitations/accept") {
        const workspace: Workspace = {
          id: "w-quinta",
          name: "Quinta do Vale",
          kind: "business",
          accent: "forest",
          role: "member",
          created_at: NOW,
        };
        state.workspaces.push(workspace);
        state.members.set(workspace.id, [{ ...ANA, role: "member" }]);
        return json(route, workspace);
      }
      const match = /^\/bff\/api\/v1\/w\/([^/]+)\/(members|invitations)(?:\/([^/]+))?$/u.exec(path);
      if (match !== null) {
        const [, id = "", kind, userId] = match;
        const members = state.members.get(id) ?? [];
        const invitations = state.invitations.get(id) ?? [];
        if (kind === "members" && method === "GET") {
          return json(route, { members });
        }
        if (kind === "members" && method === "PATCH") {
          const member = members.find((candidate) => candidate["user_id"] === userId);
          if (member !== undefined) {
            member["role"] = body["role"];
          }
          return json(route, member);
        }
        if (kind === "members" && method === "DELETE") {
          state.members.set(
            id,
            members.filter((candidate) => candidate["user_id"] !== userId),
          );
          return route.fulfill({ status: 204 });
        }
        if (kind === "invitations" && method === "GET") {
          return json(route, { invitations });
        }
        if (kind === "invitations" && method === "POST") {
          const invitation = {
            id: `i-${String(next++)}`,
            email: body["email"],
            role: body["role"] ?? "member",
            invited_by: ANA.user_id,
            created_at: NOW,
            expires_at: "2026-10-20T09:00:00Z",
          };
          state.invitations.set(id, [invitation, ...invitations]);
          // The accept link comes back once, to the inviter (demo token).
          return json(
            route,
            { ...invitation, accept_url: `${url.origin}/invitations/accept?token=demo-not-real` },
            201,
          );
        }
      }
      return json(route, { status: 404, code: "not_found", title: "Not found" }, 404);
    },
  };
}

const click = async (page: Page, name: string | RegExp) => {
  await page.getByRole("button", { name }).first().click();
};

export default async function firstRun({ page }: ScenarioContext): Promise<void> {
  const bff = fakeBff();
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.route("**/bff/**", (route) => bff.handle(route));
  // Each viewport records in a fresh browser context: a clean device.

  // Signed out: the app goes through sign-in (the fake BFF signs straight
  // in; the sign-in screens have their own scenario, `sign-in`).
  await page.goto("/");

  // First run: name, kind (with its accent), done.
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await pause(1000);
  await name.pressSequentially("Duarte & Pessoa", { delay: 60 });
  await pause(500);
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Personal" }).waitFor();
  await pause(900);
  await page.getByRole("radio", { name: "Personal" }).click();
  await pause(1200);
  await page.getByRole("radio", { name: "Business" }).click();
  await pause(900);
  await click(page, "Create workspace");
  await page.getByRole("heading", { name: "Duarte & Pessoa is ready" }).waitFor();
  await pause(1500);

  // Invite someone from Settings → Members.
  await click(page, "Invite people");
  const email = page.getByRole("textbox", { name: "Email" });
  await email.waitFor();
  await pause(1000);
  await email.pressSequentially("sofia@example.com", { delay: 50 });
  await page.getByRole("combobox", { name: "Role" }).selectOption("admin");
  await click(page, /^Invite$/u);
  await page.getByText("Invited sofia@example.com as Admin.").waitFor();
  await pause(1500);
  // Nothing emails invitations yet: copy the link (C) to send it on.
  await page.keyboard.press("c");
  await page.getByText(/Link copied/u).waitFor();
  await pause(1800);

  // Later: two people have joined. Change a role (with Undo), confirm a removal inline.
  const workspaceId = bff.state.workspaces[0]?.id ?? "";
  bff.state.members.set(workspaceId, [
    ANA,
    {
      user_id: "u-sofia",
      display_name: "Sofia Pessoa",
      email: "sofia@example.com",
      role: "admin",
      joined_at: NOW,
    },
    {
      user_id: "u-rui",
      display_name: "Rui Matos",
      email: "rui@example.com",
      role: "member",
      joined_at: NOW,
    },
  ]);
  bff.state.invitations.set(workspaceId, []);
  await page.reload();
  await page.getByText("Rui Matos", { exact: true }).waitFor();
  await pause(1200);
  await page.getByRole("combobox", { name: "Role of Rui Matos" }).selectOption("admin");
  await pause(1500);
  await click(page, "Undo");
  await pause(1500);
  await click(page, "Remove Rui Matos");
  await pause(1800);
  await click(page, "Cancel");
  await pause(800);

  // Home: nothing needs you yet, with the shortcuts that already work.
  await page.keyboard.press("g");
  await page.keyboard.press("h");
  await page.getByRole("heading", { name: "Nothing needs you yet" }).waitFor();
  await pause(2000);

  // A second workspace from the switcher (⌘⇧W): Personal, in its own accent.
  await page.keyboard.press("ControlOrMeta+Shift+W");
  await page.getByRole("dialog", { name: "Switch workspace" }).waitFor();
  await pause(1500);
  await click(page, /Create a workspace/u);
  await page.getByRole("textbox", { name: "Workspace name" }).pressSequentially("Household", {
    delay: 60,
  });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Personal" }).click();
  await pause(900);
  await click(page, "Create workspace");
  await click(page, "Open Household");
  await page.getByRole("heading", { name: "Nothing needs you yet" }).waitFor();
  await pause(1500);
  await page.keyboard.press("ControlOrMeta+Shift+W");
  await pause(1500);
  await click(page, /Duarte & Pessoa/u);
  await pause(1500);

  // An invitation link: accepted on arrival.
  await page.goto("/invitations/accept?token=demo");
  await page.getByRole("heading", { name: "You’re in Quinta do Vale" }).waitFor();
  await pause(1500);
  await click(page, "Open Quinta do Vale");
  await pause(1500);

  // The account menu: dark theme, then sign out.
  await page
    .getByRole("button", { name: /^(Ana Duarte|Account)/u })
    .first()
    .click();
  await pause(1200);
  await page.getByRole("button", { name: "Dark" }).click();
  await pause(1500);
  await click(page, "Sign out");
  await pause(1800);
}
