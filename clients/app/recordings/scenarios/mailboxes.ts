import type { Page } from "playwright";

import { pause, type ScenarioContext } from "../context.ts";
import { caption } from "./inbox.ts";

// Settings → Mailboxes on the real stack (campbooks-cloud#68): the Vite dev
// server, the real server and BFF on a scratch database, and the dev-only
// fake Google and fake Zoho of `scripts/fake-provider.ts`, whose demo Gmail
// and Zoho Mail mailboxes the connectors module really syncs. Nothing is
// intercepted. `POST /fake/expire` on the fake Zoho withdraws its grants, as
// someone revoking the app at Zoho would.
//
//   pnpm --dir clients fake-google --port 9171
//   pnpm --dir clients fake-zoho --port 9172
//   CAMPBOOKS_ENV=development CAMPBOOKS_PUBLIC_URL=http://localhost:5281 \
//   CAMPBOOKS_BIND_ADDR=127.0.0.1:8291 CAMPBOOKS_SECRET_KEY=$(openssl rand -base64 32) \
//   CAMPBOOKS_GOOGLE__CLIENT_ID=demo-client CAMPBOOKS_GOOGLE__CLIENT_SECRET=demo-secret \
//   CAMPBOOKS_GOOGLE__BASE_URL=http://127.0.0.1:9171 \
//   CAMPBOOKS_ZOHO__CLIENT_ID=demo-zoho-client CAMPBOOKS_ZOHO__CLIENT_SECRET=demo-zoho-secret \
//   CAMPBOOKS_ZOHO__BASE_URL=http://127.0.0.1:9172 CAMPBOOKS_BLOBS__PATH=/tmp/blobs \
//   CAMPBOOKS_DATABASE_URL=postgres://…/scratch cargo run -p campbooks-server -- all
//   CAMPBOOKS_API_URL=http://127.0.0.1:8291 pnpm --dir clients/app exec vite --port 5281
//   pnpm --dir clients record mailboxes --base-url http://localhost:5281
//
// Sign up with Google (its Gmail connects), open Settings → Mailboxes,
// connect Zoho Mail, see it need you after Zoho withdraws the grant,
// reconnect it, disconnect it (Zoho stays a way in), and, with no mailbox
// left, Home's "Connect your mail" leads back here. Fictional person and
// mail only; no token is ever shown.

const fakeZoho = process.env["FAKE_ZOHO_URL"] ?? "http://127.0.0.1:9172";

function isPhone(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1440) < 768;
}

/** Allow on the fake provider's consent pages, as many as it asks. */
async function allowAll(page: Page, provider: string): Promise<void> {
  await page.getByText(`Fake ${provider} · development only`).waitFor();
  await caption(page, `${provider}'s consent (a dev-only fake): Allow`, 1400);
  // The fakes listen on 127.0.0.1; the app on localhost.
  while (new URL(page.url()).hostname === "127.0.0.1") {
    await page.getByRole("link", { name: "Allow" }).click();
    await page.waitForLoadState();
    await pause(500);
  }
}

async function openMailboxes(page: Page, phone: boolean): Promise<void> {
  if (phone) {
    await page.getByRole("link", { name: "Settings" }).last().click();
    await page.getByRole("button", { name: "Mailboxes", exact: true }).click();
  } else {
    await page.getByRole("link", { name: "Settings" }).first().click();
    await page.getByRole("link", { name: "Mailboxes", exact: true }).click();
  }
  await page.getByRole("heading", { level: 1, name: "Mailboxes" }).waitFor();
}

/** Press `key` on desktop, or click `button` on a phone. */
async function act(page: Page, phone: boolean, key: string, button: RegExp): Promise<void> {
  await (phone ? page.getByRole("button", { name: button }).click() : page.keyboard.press(key));
}

export default async function mailboxes({ page }: ScenarioContext): Promise<void> {
  const phone = isPhone(page);

  // Sign up with Google: an account, a workspace, its Gmail.
  await page.goto("/");
  await caption(page, "Sign up with Google");
  await page.getByRole("link", { name: /Continue with Google/u }).click();
  await allowAll(page, "Google");
  await page.getByRole("button", { name: /Create account/u }).click();
  const name = page.getByRole("textbox", { name: "Workspace name" });
  await name.waitFor();
  await name.fill("Rivera Studio");
  await page.keyboard.press("Enter");
  await page.getByRole("radio", { name: "Business" }).click();
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByRole("heading", { name: /will be connected to this workspace/u }).waitFor();
  await page.keyboard.press("Enter");
  await page.getByRole("heading", { name: "Your mail" }).waitFor();
  await caption(page, "Home: the Gmail from sign-up, syncing — and Manage mailboxes", 2200);

  // Settings → Mailboxes.
  await openMailboxes(page, phone);
  await caption(
    page,
    phone
      ? "Settings → Mailboxes (the sections are chips on a phone)"
      : "Settings → Mailboxes: provider, who connected it, and how it's doing",
    2200,
  );

  // Connect Zoho Mail through the front door, and come back here.
  if (!phone) {
    await caption(page, "⇧C jumps to Connect", 1200);
    await page.keyboard.press("Shift+C");
    await pause(500);
    await page.keyboard.press("Tab");
    await pause(700);
  }
  await page.getByRole("button", { name: /Connect Zoho Mail/u }).click();
  await allowAll(page, "Zoho");
  await page.getByText(/^Zoho Mail .+ is connected$/u).waitFor();
  await caption(page, "Back on Mailboxes: Zoho Mail is connected and syncing", 2400);

  // Zoho withdraws the grant: the mailbox needs you.
  await page.request.post(`${fakeZoho}/fake/expire`);
  await caption(page, "Meanwhile, the grant is withdrawn at Zoho…", 1400);
  const { workspaces } = await page.evaluate<{ workspaces: { id: string }[] }>(
    `fetch("/bff/api/v1/workspaces", { headers: { "x-campbooks-csrf": "1" } }).then((r) => r.json())`,
  );
  const workspace = workspaces[0]?.id ?? "";
  const { connections } = await page.evaluate<{ connections: { id: string; provider: string }[] }>(
    `fetch("/bff/api/v1/w/${workspace}/connections", { headers: { "x-campbooks-csrf": "1" } }).then((r) => r.json())`,
  );
  const zoho = connections.find((connection) => connection.provider === "zoho")?.id ?? "";
  // Ask for a sync now (again, until it has run): it finds the grant gone.
  const needsYou = page.getByText("Needs you to sign in again");
  for (let attempt = 0; attempt < 60 && !(await needsYou.isVisible()); attempt += 1) {
    await page.evaluate(
      `fetch("/bff/api/v1/w/${workspace}/connections/${zoho}/sync", { method: "POST", headers: { "x-campbooks-csrf": "1" } })`,
    );
    await pause(3000);
  }
  await needsYou.waitFor();
  await caption(
    page,
    phone ? "…so it needs you: Reconnect" : "…so it needs you: R reconnects the chosen mailbox",
    2400,
  );
  await act(page, phone, "r", /^Reconnect /u);
  await allowAll(page, "Zoho");
  await page.getByText(/^Zoho Mail .+ is connected$/u).waitFor();
  await page.getByText("Needs you to sign in again").waitFor({ state: "detached" });
  await caption(page, "Reconnected: syncing again", 2000);

  // Disconnect Zoho Mail: confirmed in place; the way in stays.
  if (!phone) {
    await caption(page, "J and K choose a mailbox; ⌫ disconnects it", 1600);
  }
  const zohoRow = page.getByRole("listitem").filter({ hasText: "Zoho Mail ·" });
  if (phone) {
    await zohoRow.getByRole("button", { name: /^Disconnect/u }).click();
  } else {
    await page.keyboard.press("j");
    await pause(500);
    await page.keyboard.press("Backspace");
  }
  await page.getByText(/signing in with this account keeps working/u).waitFor();
  await caption(
    page,
    "It says what happens: the mail stays, and so does signing in with Zoho",
    2800,
  );
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await page.getByText(/is disconnected$/u).waitFor();
  await caption(page, "Disconnected", 1400);

  // Ways to sign in: Zoho is still one.
  await page.goto(`/w/${workspace}/settings/sign-in`);
  await page.getByRole("heading", { level: 1, name: "Ways to sign in" }).waitFor();
  await page.getByText("Zoho", { exact: true }).waitFor();
  await caption(page, "Ways to sign in: Zoho is still a way in", 2400);

  // Disconnect Gmail too, then Home's "Connect your mail" leads back here.
  await openMailboxes(page, phone);
  await page
    .getByRole("button", { name: /^Disconnect/u })
    .first()
    .click();
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await page.getByText("No mailboxes yet").waitFor();
  await page.goto(`/w/${workspace}`);
  await page.getByRole("heading", { name: "Connect your mail" }).waitFor();
  await caption(page, "No mailbox left: Home offers Connect your mail…", 2000);
  await page.getByRole("link", { name: /Connect your mail/u }).click();
  await page.getByRole("heading", { level: 1, name: "Mailboxes" }).waitFor();
  await caption(page, "…which opens Settings → Mailboxes", 2400);
}
