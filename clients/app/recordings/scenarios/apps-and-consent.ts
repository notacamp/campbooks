import { createHash, randomBytes } from "node:crypto";

import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";

// Apps other than Campbooks on the web (ADR 0014, slice S4), end to end on
// the real stack: the Vite dev server, the real server and BFF on a scratch
// database, and the dev-only fake Google. Nothing is intercepted.
//
//   pnpm --dir clients fake-google
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5173 \
//   CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9050 \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- api
//   pnpm --dir clients dev
//   pnpm --dir clients record apps-and-consent
//
// Sign up, register the workspace's own server-to-server app (its secret
// shown once), then an MCP-style agent registers itself and the person
// allows it on the consent screen in one workspace, sees it under connected
// apps, and revokes it. Fictional person and demo secret only.

/** A caption pinned to the bottom of the page, so the video explains itself. */
async function caption(page: Page, text: string): Promise<void> {
  // A script string: the recordings are type-checked without the DOM types.
  await page.evaluate(`(() => {
    const box = document.getElementById("recording-caption") ?? document.createElement("div");
    box.id = "recording-caption";
    box.textContent = ${JSON.stringify(text)};
    Object.assign(box.style, {
      position: "fixed", left: "12px", right: "12px", bottom: "12px",
      zIndex: "2147483647", padding: "10px 14px", borderRadius: "10px",
      background: "rgba(20, 20, 24, 0.88)", color: "#fff",
      font: "500 15px/1.4 system-ui, sans-serif", pointerEvents: "none",
    });
    document.body.append(box);
  })()`);
  await pause(1800);
}

/** Open the account menu (the avatar on a phone, the footer on desktop). */
async function accountMenu(page: Page): Promise<void> {
  await page
    .getByRole("button", { name: /^(Sam Rivera|Account)/u })
    .first()
    .click();
  await pause(700);
}

async function openApps(page: Page): Promise<void> {
  await accountMenu(page);
  await page.getByRole("link", { name: "Apps and integrations" }).click();
  await page.getByRole("heading", { level: 1, name: "Apps and integrations" }).waitFor();
}

export default async function appsAndConsent({ page }: ScenarioContext): Promise<void> {
  // Sign up with Google: the account and a workspace.
  await page.goto("/");
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await page.getByRole("link", { name: "Allow" }).click();
  await page.getByRole("button", { name: /Create account/u }).waitFor();
  await page.keyboard.press("Enter");
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.pressSequentially("Duarte & Pessoa", { delay: 30 });
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("button", { name: /Not now/u }).click();
  await caption(page, "Signed up, owner of Duarte & Pessoa. Account → Apps and integrations");

  // The workspace's own app: a server-to-server export with a secret.
  await openApps(page);
  await caption(page, "Nothing let in yet, and no apps of the workspace's own");
  await page.keyboard.press("Shift+N");
  await page.getByRole("textbox", { name: "Name" }).pressSequentially("Nightly export", {
    delay: 40,
  });
  await caption(page, "Server to server, read only: it gets a client ID and a secret");
  await page.getByRole("button", { name: "Create app" }).click();
  await page.getByText("Copy the secret now").waitFor();
  await caption(page, "The secret is shown this once (stored only as a hash), with Copy");
  await page.getByRole("button", { name: "Done" }).click();
  await pause(800);

  // An agent registers itself (RFC 7591) and asks this browser for consent.
  const registered = await page.request.post("/oauth2/register", {
    data: {
      client_name: "Desk Agent",
      redirect_uris: ["http://localhost:5173/"],
      grant_types: ["authorization_code", "refresh_token"],
      scope: "conversations:read offline_access",
    },
  });
  const { client_id: clientId } = (await registered.json()) as { client_id: string };
  const verifier = randomBytes(32).toString("base64url");
  const challenge = createHash("sha256").update(verifier).digest("base64url");
  const authorize = new URLSearchParams({
    response_type: "code",
    client_id: clientId,
    redirect_uri: "http://localhost:5173/",
    scope: "conversations:read offline_access",
    state: "agent-state",
    code_challenge: challenge,
    code_challenge_method: "S256",
    resource: "http://localhost:5173/api",
  });
  await page.goto(`/oauth2/authorize?${authorize.toString()}`);
  await page.getByRole("heading", { name: /Desk Agent wants to use/u }).waitFor();
  await caption(page, "An agent registered itself: the consent screen marks it unverified");
  await page.mouse.wheel(0, 600);
  await caption(page, "What it may do, in plain words, and on its own device: asked every time");
  await page.getByRole("radio", { name: "Duarte & Pessoa" }).click();
  await caption(
    page,
    "Nothing is picked for you, and Allow waits a second. Its token reaches this workspace only",
  );
  await page.getByRole("button", { name: "Allow" }).click();
  await page.waitForURL(/code=/u);
  await caption(page, "Back to the agent with a one-time code");

  // Connected apps: the agent, where and what; Revoke ends its access.
  await openApps(page);
  await page.getByText("Desk Agent").first().waitFor();
  await caption(page, "Connected apps: the agent, unverified, in Duarte & Pessoa");
  await page.getByRole("button", { name: "Revoke Desk Agent" }).click();
  await caption(page, "Revoke asks first, in place");
  await page.getByRole("button", { name: "Revoke", exact: true }).click();
  await page.getByText("No app can act as you.").waitFor();
  await caption(page, "Revoked: its refresh tokens stopped working at once");
  await pause(1200);
}
