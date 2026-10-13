import type { Page, Route } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Signing up on the real app (Vite dev server): the sign-in screen, a demo
// stand-in for the provider's consent, the new-account screen, the first
// workspace, the mailbox step and Home. The server (BFF, issuer and API) is
// answered in the browser from in-memory demo data; nothing leaves the page.
//   pnpm --dir clients dev
//   pnpm --dir clients record sign-in
// Fictional people and addresses only.

const NOW = "2026-10-13T09:00:00Z";
const REQUEST = "req-demo";
const ANA = { name: "Ana Duarte", email: "ana@example.com" };
const GRANT = {
  id: "0192f1a0-0000-7000-8000-0000000000b1",
  provider: "google",
  address: ANA.email,
  display_name: ANA.name,
  expires_at: "2026-10-20T09:00:00Z",
};

/** A neutral page standing in for the provider's consent (never imitates it). */
const DEMO_PROVIDER = `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1"><title>Demo provider</title>
<style>body{font:16px/1.5 system-ui,sans-serif;margin:0;display:grid;place-items:center;min-height:100vh;background:#f4f4f2;color:#222}
main{max-width:22rem;padding:2rem;text-align:center}a{display:inline-block;margin-top:1rem;padding:.75rem 1.5rem;border-radius:.5rem;background:#222;color:#fff;text-decoration:none}</style></head>
<body><main><p><strong>Demo provider</strong></p><p>In this recording, this page stands in for the provider’s own consent screen, where Ana allows sign-in and Gmail access.</p>
<a href="/auth/callback/google?code=demo&amp;state=demo">Allow</a></main></body></html>`;

function fakeServer() {
  const state = {
    signedIn: false,
    held: [] as (typeof GRANT)[],
    connections: [] as Record<string, unknown>[],
    workspaces: [] as Record<string, unknown>[],
  };
  const json = (route: Route, body: unknown, status = 200) =>
    route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
  const redirect = (route: Route, location: string) =>
    route.fulfill({ status: 303, headers: { location } });

  return {
    async handle(route: Route) {
      const request = route.request();
      const url = new URL(request.url());
      const method = request.method();
      const path = url.pathname;
      const body = (request.postDataJSON() ?? {}) as Record<string, string>;

      // The BFF starts a sign-in: the issuer has no session, so it sends the
      // browser to the sign-in screen with a waiting request.
      if (path === "/bff/login") {
        return redirect(route, `/sign-in?request=${REQUEST}`);
      }
      if (path === `/auth/requests/${REQUEST}`) {
        return json(route, { client_id: "campbooks-web", providers: ["google"] });
      }
      if (path === "/auth/start/google") {
        return route.fulfill({ status: 200, contentType: "text/html", body: DEMO_PROVIDER });
      }
      // Back from the provider with an identity no account uses yet.
      if (path === "/auth/callback/google") {
        return redirect(route, `/sign-in/new?request=${REQUEST}`);
      }
      if (path === "/auth/new-account" && method === "GET") {
        return json(route, { provider: "google", email: ANA.email, display_name: ANA.name });
      }
      if (path === "/auth/new-account" && method === "POST") {
        state.held = [GRANT];
        return json(route, { redirect: "/bff/callback?code=demo&state=demo" }, 201);
      }
      if (path === "/bff/callback") {
        state.signedIn = true;
        return redirect(route, "/");
      }
      if (path === "/bff/session") {
        return json(
          route,
          state.signedIn
            ? { authenticated: true, user: { sub: "ana", name: ANA.name, email: ANA.email } }
            : { authenticated: false },
        );
      }
      if (path === "/bff/api/v1/workspaces" && method === "GET") {
        return json(route, { workspaces: state.workspaces });
      }
      if (path === "/bff/api/v1/workspaces" && method === "POST") {
        const personal = body["kind"] === "personal";
        const workspace = {
          id: "w-1",
          name: body["name"] ?? "My workspace",
          kind: personal ? "personal" : "business",
          accent: personal ? "personal" : "business",
          role: "owner",
          created_at: NOW,
        };
        state.workspaces.push(workspace);
        return json(route, workspace, 201);
      }
      if (path === "/bff/api/v1/identity/held-grants") {
        return json(route, { items: state.held });
      }
      if (path === `/bff/api/v1/w/w-1/identity/held-grants/${GRANT.id}/adopt`) {
        state.held = [];
        state.connections = [
          {
            id: "c-1",
            provider: "google",
            address: ANA.email,
            address_verified: true,
            display_name: ANA.name,
            granted_by: "u-ana",
            capabilities: ["messages"],
            health: "ok",
            health_changed_at: NOW,
            health_reason: null,
            streams: [],
            created_at: NOW,
          },
        ];
        return json(route, { connection: "c-1" });
      }
      if (path === "/bff/api/v1/w/w-1/connections") {
        return json(route, { connections: state.connections });
      }
      if (path === "/bff/api/v1/w/w-1/members") {
        return json(route, { members: [] });
      }
      return json(route, { status: 404, code: "not_found", title: "Not found" }, 404);
    },
  };
}

const button = (page: Page, name: string | RegExp) => page.getByRole("button", { name }).first();

export default async function signIn({ page }: ScenarioContext): Promise<void> {
  const server = fakeServer();
  for (const pattern of ["**/bff/**", "**/auth/**"]) {
    await page.route(pattern, (route) => server.handle(route));
  }

  // A signed-out browser opens Campbooks and lands on the sign-in screen.
  await page.goto("/");
  const google = page.getByRole("link", { name: /Continue with Google/u });
  await google.waitFor();
  await pause(1800);
  // What one consent covers, read before leaving.
  await page.getByText("What Campbooks reads and keeps").click();
  await pause(3500);

  // 1: Continue with Google (a demo stand-in for its consent screen).
  await page.keyboard.press("1");
  const allow = page.getByRole("link", { name: "Allow" });
  await allow.waitFor();
  await pause(1500);
  await allow.click();

  // No account uses this Google account yet: who it's for, what happens next.
  const create = button(page, /Create account/u);
  await create.waitFor();
  await pause(3000);
  await page.keyboard.press("Enter");

  // First run: name the workspace, pick what it's for, create it.
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await pause(800);
  await name.pressSequentially("Duarte & Pessoa", { delay: 60 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).waitFor();
  await pause(800);
  await page.getByRole("radio", { name: "Business" }).click();
  await pause(600);
  await button(page, "Create workspace").click();

  // The mailbox from sign-up: connect it here (Enter).
  await page
    .getByRole("heading", {
      name: "Your Gmail ana@example.com will be connected to this workspace",
    })
    .waitFor();
  await pause(3500);
  await page.keyboard.press("Enter");

  // Home names the connected mailbox.
  await page.getByRole("heading", { name: "Your mail" }).waitFor();
  await pause(3000);
}
